//! Una vuelta del servicio, fuera del `main`.
//!
//! RPT-037, PA-68.
//!
//! # Por que existe este modulo
//!
//! El ciclo vivia en `main.rs`, el unico fichero del workspace sin pruebas. Ahi
//! nacio el defecto de RPT-036 §3: `let instante = ahora();` se calculaba **una
//! vez antes del bucle**, con lo que en un demonio de dias el reloj quedaba
//! congelado y **ningun marcado caducaba nunca**.
//!
//! Ninguna prueba lo vio porque ninguna ejecutaba dos vueltas. Y ninguna podia:
//! el ciclo estaba pegado a la captura, que exige una tarjeta de red.
//!
//! # Las dos costuras que lo hacen probable
//!
//! **La captura sale del ciclo.** [`Ciclo::vuelta`] recibe lo observado en lugar
//! de observarlo. Es la misma disciplina que [`Despacho`](crate::salida::Despacho)
//! y [`Atiende`](crate::servicio::Atiende): el I/O detras de un parametro.
//!
//! **El reloj es un parametro.** No hay ninguna llamada a `SystemTime::now` aqui
//! dentro. Una prueba puede adelantarlo entre vueltas, y congelarlo ya no es un
//! descuido invisible: exige pasar el mismo valor dos veces a proposito.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use eje_almacen::RegistroEvidencia;
use eje_ipc::mensajes::{Condiciones, PeticionAlertas};
use guardian_cc::arranque::EstadoArranque;
use guardian_cc::clasificacion::{Evidencia, MarcadoDispositivo, clasificar};
use guardian_cc::observacion::{AlmacenObservacion, Protocolo};
use guardian_cc::proveedores::{
    DireccionEnlace, Indicio, ProveedorHuella, ProveedorInventario, ProveedorSegmento,
};
use guardian_cc::{PerfilSegmento, Veredicto, evaluar};

use crate::alertas::{anotar_incontenible, condiciones, consultar, persistir, rotar_si_toca};
use crate::salida::{Despacho, Emisor};

/// Una trama ya interpretada, tal como el ciclo la consume.
///
/// El ciclo **no captura**: recibe esto. Sin esa costura, probarlo exigiria una
/// tarjeta de red, y por eso no habia pruebas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observacion {
    /// Origen de la trama.
    pub origen: DireccionEnlace,
    /// Protocolo que los puertos sugieren, si sugieren alguno.
    pub protocolo: Option<Protocolo>,
    /// Etiqueta VLAN, si la trama la llevaba.
    pub vlan: Option<u16>,
}

/// Lo que una vuelta produjo.
#[derive(Debug, Clone)]
pub struct Resultado {
    /// Condiciones vigentes al terminar, con el resultado de la salida ya
    /// incorporado.
    pub condiciones: Condiciones,
    /// Dispositivos con marcado firmado.
    pub con_marcado: u64,
    /// Contenibles sin intervencion humana.
    pub contenibles: u64,
    /// Los que exigen humano o estan prohibidos.
    pub escalados: u64,
    /// Alertas anexadas **en esta vuelta**, ya emitidas.
    pub anexadas: Vec<eje_ipc::mensajes::SucesoAlerta>,
    /// Si esta vuelta escribio el registro a disco.
    ///
    /// Falso cuando nada cambio: RPT-034 §1.1. En un sensor tranquilo no se
    /// escribe nunca, y eso evita el coste cuadratico.
    pub persistido: bool,
    /// Motivo por el que no se pudo persistir, si hubo alertas y fallo.
    ///
    /// Se devuelve en lugar de imprimirse porque quien llama decide si eso es un
    /// mensaje de consola o una condicion. Ver PA-69.
    pub fallo_persistencia: Option<String>,
    /// Amenazas detectadas que **no cupieron** en el registro.
    ///
    /// Distinto de `fallo_persistencia`: ahi la alerta existe y no llego al
    /// disco; aqui no llego a existir. Ver PA-72.
    pub perdidas: u64,
    /// Segmento archivado en esta vuelta, si se alcanzo el umbral.
    ///
    /// Ver PA-59. Que sea `None` es lo normal: se rota una vez cada
    /// `ASIENTOS_POR_SEGMENTO` alertas.
    pub rotado: Option<PathBuf>,
    /// El extremo del registro salio hacia el testigo externo en esta vuelta.
    ///
    /// Falso tambien cuando no habia nada que sellar —registro vacio, extremo sin
    /// cambios, o sin colector configurado—, que no es un fallo. Ver PA-64.
    pub sellado: bool,
}

impl Resultado {
    /// Hay alertas de esta vuelta que solo viven en memoria.
    ///
    /// Es la unica perdida de evidencia que el ciclo puede detectar por si
    /// mismo, y todavia no tiene canal propio hacia el operador: PA-69.
    ///
    /// Se lee de la condicion, que es ahora la fuente unica: el ciclo recuerda
    /// el riesgo entre vueltas, asi que sigue siendo cierto aunque esta vuelta no
    /// haya anexado nada.
    #[must_use]
    pub const fn evidencia_en_riesgo(&self) -> bool {
        self.condiciones.evidencia_en_riesgo
    }
}

/// Estado que sobrevive de una vuelta a la siguiente.
///
/// # Que vive aqui y por que
///
/// **El almacen de observacion**, porque recrearlo borraria la ambiguedad
/// pegajosa de RPT-010 §5 y con ella la proteccion del carro de telemedicina que
/// paso por la VLAN clinica. Un demonio que olvida cada minuto es peor que un
/// recorrido que recuerda una vez.
///
/// **El emisor**, porque guarda las condiciones del ciclo anterior; sin ellas
/// repetiria la misma transicion en cada vuelta (RPT-032 §3).
///
/// **El registro**, porque la serie de asientos no puede reiniciarse.
pub struct Ciclo<D> {
    almacen: AlmacenObservacion,
    registro: RegistroEvidencia,
    emisor: Option<Emisor<D>>,
    evidencia: PathBuf,
    perfil: PerfilSegmento,
    /// Vueltas que el registro lleva por delante del fichero.
    ///
    /// RPT-044, PA-69. `Some(n)` significa que hubo una escritura fallida hace
    /// `n` vueltas y que desde entonces lo anexado **solo vive en memoria**.
    ///
    /// No es una cola: las alertas ya estan en `registro`, que es su sitio. Una
    /// cola aparte seria el agotamiento de memoria de RPT-018 §6 con otro nombre,
    /// y ademas con el disco lleno crecer empeora el siguiente intento.
    riesgo_desde: Option<u64>,
    /// El sensor no esta observando. RPT-047, PA-81.
    ///
    /// Vive en el ciclo y no se deriva del almacen porque el almacen solo sabe
    /// de lo que llego; que la captura no se pueda abrir es un hecho de fuera.
    captura_no_disponible: bool,
}

impl<D: Despacho> Ciclo<D> {
    /// Ciclo sobre el registro ya cargado del disco.
    #[must_use]
    pub fn nuevo(
        evidencia: PathBuf,
        perfil: PerfilSegmento,
        registro: RegistroEvidencia,
        emisor: Option<Emisor<D>>,
    ) -> Self {
        Self {
            almacen: AlmacenObservacion::nuevo(),
            registro,
            emisor,
            evidencia,
            perfil,
            riesgo_desde: None,
            captura_no_disponible: false,
        }
    }

    /// Declara si el sensor esta observando. RPT-047 §4, PA-81.
    ///
    /// # Por que es un interruptor y no un dato del almacen
    ///
    /// Que la captura este abierta es un hecho de fuera del ciclo: lo sabe quien
    /// intenta abrirla. El almacen solo sabe de lo que llego, y **lo que llego
    /// cuando nadie mira es identico a lo que llega en una red tranquila**: cero
    /// tramas. Sin este interruptor las dos situaciones son indistinguibles
    /// desde dentro, y el panel pintaria una observacion normal con el sensor
    /// ciego.
    ///
    /// Se fija en cada vuelta, antes de la vuelta, incluso cuando no cambia: un
    /// estado que solo se fija al cambiar es un estado que se queda pegado si
    /// alguien olvida el camino de vuelta.
    pub const fn declarar_captura(&mut self, disponible: bool) {
        self.captura_no_disponible = !disponible;
    }

    /// Registro tal como esta ahora.
    #[must_use]
    pub const fn registro(&self) -> &RegistroEvidencia {
        &self.registro
    }

    /// Almacen de observacion, para el resumen por pantalla.
    #[must_use]
    pub const fn almacen(&self) -> &AlmacenObservacion {
        &self.almacen
    }

    /// Ruta del registro persistido.
    #[must_use]
    pub fn evidencia(&self) -> &Path {
        &self.evidencia
    }

    /// Anota que la captura perdio tramas.
    ///
    /// Va aparte de [`Self::vuelta`] porque la perdida la reporta el nucleo, no
    /// las tramas: quien captura la conoce y el ciclo no puede deducirla.
    pub const fn anotar_perdida(&mut self) {
        self.almacen.anotar_perdida();
    }

    /// Escribe el registro si hace falta y lleva la cuenta del riesgo.
    ///
    /// RPT-044, PA-69. Devuelve si el fichero quedo al dia y el motivo del fallo.
    ///
    /// # Se escribe si anexo **o si lo de antes no llego**
    ///
    /// Antes la guarda era solo lo primero, y ahi vivia la perdida de verdad: una
    /// vuelta anexaba, la escritura fallaba, las siguientes no anexaban nada
    /// —lo normal en un sensor tranquilo— y **nadie volvia a intentarlo**. El
    /// disco se recuperaba a los diez segundos y el agente seguia con las alertas
    /// solo en memoria hasta la amenaza siguiente.
    ///
    /// # El asiento de constancia se anexa al RECUPERAR
    ///
    /// Uno escrito durante el fallo iria al registro que no se puede escribir y
    /// moriria con el proceso, igual que las alertas que pretende explicar.
    /// Ademas, con el disco lleno, anadir bytes empeora el intento siguiente.
    ///
    /// Vive aparte de [`Self::vuelta`] para poder ejercitarlo con una ruta que
    /// falla de verdad, sin necesidad de un inventario firmado que produzca
    /// veredictos prohibidos.
    fn asegurar_durabilidad(
        &mut self,
        hubo_anexado: bool,
        ahora_ms: i64,
    ) -> (bool, Option<String>) {
        if !hubo_anexado && self.riesgo_desde.is_none() {
            return (false, None);
        }

        match persistir(&self.evidencia, &self.registro) {
            Err(error) => {
                self.riesgo_desde = Some(self.riesgo_desde.unwrap_or(0) + 1);
                (false, Some(error.to_string()))
            }
            Ok(()) => {
                let Some(vueltas) = self.riesgo_desde.take() else {
                    return (true, None);
                };

                let _ = self.registro.anexar(
                    ahora_ms,
                    eje_almacen::ClaseEvento::PersistenciaRestablecida,
                    "almacen",
                    &format!(
                        "la escritura fallo durante {vueltas} vuelta(s); hasta ahora \
                         las alertas solo vivian en memoria"
                    ),
                );

                // Segunda escritura, solo al recuperar: el asiento que da fe del
                // tramo tiene que ser durable el mismo.
                match persistir(&self.evidencia, &self.registro) {
                    Ok(()) => (true, None),
                    Err(error) => {
                        self.riesgo_desde = Some(0);
                        (false, Some(error.to_string()))
                    }
                }
            }
        }
    }

    /// Ejecuta una vuelta completa.
    ///
    /// # El orden es el de RPT-034 §4 y no es negociable
    ///
    /// Observar → clasificar → anexar → **persistir** → emitir. Persistir va
    /// antes de emitir: si el proceso muere entre ambos, la alerta esta en disco
    /// y el SIEM no se entero —recuperable—; al reves, el SIEM sabe de una
    /// alerta que el registro no tiene, y eso es peor que no saber.
    ///
    /// # Solo se emite lo de esta vuelta
    ///
    /// La marca de agua se toma del **numero del ultimo asiento** antes de
    /// anexar, no de la longitud: cuando PA-59 pode el registro, longitud y
    /// numero dejaran de coincidir y consultar por longitud reemitiria alertas
    /// viejas.
    pub fn vuelta(
        &mut self,
        estado: &EstadoArranque,
        observaciones: &[Observacion],
        ahora_s: u64,
        ahora_ms: i64,
    ) -> Resultado {
        let mut vistos: BTreeMap<DireccionEnlace, u64> = BTreeMap::new();

        for observacion in observaciones {
            self.almacen.observar(
                observacion.origen,
                observacion.protocolo,
                estado.declaracion_para(observacion.vlan, ahora_s),
            );
            *vistos.entry(observacion.origen).or_insert(0) += 1;
        }

        let marca = self
            .registro
            .asientos()
            .last()
            .map_or(0, |asiento| asiento.numero);

        let mut con_marcado = 0u64;
        let mut contenibles = 0u64;
        let mut escalados = 0u64;
        let mut perdidas = 0u64;

        for mac in vistos.keys() {
            let Ok(historial) = self.almacen.historial(mac) else {
                escalados = escalados.saturating_add(1);
                continue;
            };

            // Un fallo de esta fuente **declarativa** escala en lugar de leerse
            // como ausencia de marcado (RPT-010 §4).
            let Ok(marcado) = estado.marcado(mac) else {
                escalados = escalados.saturating_add(1);
                continue;
            };

            if marcado.is_some() {
                con_marcado = con_marcado.saturating_add(1);
            }

            let evidencia = Evidencia {
                marcado: marcado.map(|marcado| MarcadoDispositivo {
                    clase: marcado.clase(),
                    // Aqui hacia dano el reloj congelado: con `ahora_s` fijo,
                    // `vigente` era eternamente cierto.
                    vigente: marcado.vigente_en(ahora_s),
                }),
                segmento: historial.declaracion_efectiva(),
                inferencia: self
                    .almacen
                    .indicio(mac)
                    .unwrap_or(Indicio::Indeterminado)
                    .clase(),
            };

            let veredicto = evaluar(clasificar(&evidencia), self.perfil);

            if veredicto.es_amenaza_incontenible()
                && anotar_incontenible(&mut self.registro, ahora_ms, mac, &veredicto).is_none()
            {
                // PA-72. El registro esta lleno: la amenaza se detecto y **no
                // queda constancia de ella**. La condicion `registroSaturado` lo
                // dice, pero este contador dice cuantas se perdieron, que es lo
                // que un auditor preguntara.
                perdidas = perdidas.saturating_add(1);
            }

            match veredicto {
                Veredicto::Ejecutar => contenibles = contenibles.saturating_add(1),
                _ => escalados = escalados.saturating_add(1),
            }
        }

        // En lotes, porque `consultar` acota a `SUCESOS_POR_CONSULTA` para que la
        // respuesta quepa en un marco de IPC. Esa cota protege al **canal**, no a
        // la salida: tomar solo el primer lote dejaria sin emitir las alertas a
        // partir de la 257 de una misma vuelta, y la marca de agua de la vuelta
        // siguiente pasaria por encima de ellas. Nunca saldrian.
        let mut anexadas = Vec::new();
        let mut avance = marca;
        loop {
            let lote = consultar(
                &self.registro,
                &PeticionAlertas {
                    desde_asiento: avance,
                },
            );
            // `consultar` filtra por `> desde_asiento`, asi que el ultimo asiento
            // del lote es estrictamente mayor que `avance` y el bucle progresa.
            let Some(ultimo) = lote.last() else { break };
            avance = ultimo.asiento;
            anexadas.extend(lote);
        }

        let (persistido, fallo_persistencia) =
            self.asegurar_durabilidad(!anexadas.is_empty(), ahora_ms);
        let mut fallo_persistencia = fallo_persistencia;
        let mut rotado = None;

        // PA-59. Se rota **despues** de persistir y solo si lo persistido llego a
        // disco: rotar sobre un activo que no se pudo escribir archivaria una
        // copia buena y dejaria el activo vacio, perdiendo lo que no cupo.
        if persistido {
            match rotar_si_toca(&self.evidencia, &mut self.registro) {
                Ok(destino) => rotado = destino,
                Err(error) => fallo_persistencia = Some(error.to_string()),
            }
        }

        // Se emite con las condiciones SIN el campo de salida —que aun no se
        // conoce— y el resultado del envio lo rellena despues.
        // `salidaNoDisponible` no viaja nunca por syslog, asi que el orden no
        // altera lo que sale (RPT-032 §4).
        let base = condiciones(
            estado,
            &self.almacen,
            &self.registro,
            self.captura_no_disponible,
        );
        let mut salida_bien = self
            .emisor
            .as_mut()
            .is_none_or(|emisor| emisor.emitir(&anexadas, &base, ahora_ms));

        // PA-64. El sello describe lo que hay **en disco**, no lo que hay en
        // memoria. Si se anexo y la escritura fallo, callar es lo correcto:
        // anunciar un extremo que no sobrevive al reinicio haria que el arranque
        // siguiente pareciera un recorte, y el testigo acusaria de manipulacion
        // a un fallo de disco.
        //
        // Cuando no se anexo nada, en cambio, el extremo vigente **si** es el del
        // disco —viene de ahi— y sellarlo es lo que da la linea base tras cada
        // arranque. Esa linea base es justamente la que detecta la manipulacion
        // hecha con el agente parado (RPT-038 §4).
        let durable = anexadas.is_empty() || persistido;

        // Se sella por `ultimo_numero` y `extremo`, no por el ultimo asiento
        // presente. Tras rotar, el segmento activo esta vacio y aun asi hay algo
        // que atestiguar: el extremo del segmento cerrado, que este arrastra como
        // genesis. Mirar el ultimo asiento daria `None` y el testigo perderia
        // justo el sello de la frontera entre segmentos.
        let ultimo = self.registro.ultimo_numero();
        let sellado = match (durable && ultimo > 0, &mut self.emisor) {
            (true, Some(emisor)) => {
                let entregado =
                    emisor.sellar(ultimo, &self.registro.extremo().hexadecimal(), ahora_ms);
                if !entregado {
                    salida_bien = false;
                }
                entregado
            }
            _ => false,
        };

        Resultado {
            condiciones: Condiciones {
                salida_no_disponible: !salida_bien,
                evidencia_en_riesgo: self.riesgo_desde.is_some(),
                ..base
            },
            con_marcado,
            contenibles,
            escalados,
            anexadas,
            persistido,
            fallo_persistencia,
            sellado,
            perdidas,
            rotado,
        }
    }
}

#[cfg(test)]
mod pruebas {
    // Mismo encabezado que el resto de modulos de prueba del workspace.
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::alertas::{CargaRegistro, cargar_desde};
    use crate::salida::ErrorSalida;
    use guardian_cc::ClaseExcluida;

    const MAC: DireccionEnlace = [0x00, 0x1B, 0x21, 0x00, 0x00, 0x01];
    const OTRA: DireccionEnlace = [0x00, 0x1B, 0x21, 0x00, 0x00, 0x02];

    /// Despacho que guarda lo emitido en un buzon compartido con la prueba.
    struct DespachoEspia {
        buzon: Rc<RefCell<Vec<String>>>,
    }

    impl Despacho for DespachoEspia {
        fn enviar(&mut self, marco: &[u8]) -> Result<(), ErrorSalida> {
            self.buzon
                .borrow_mut()
                .push(String::from_utf8_lossy(marco).into_owned());
            Ok(())
        }
    }

    /// Directorio de prueba que se limpia al soltarse.
    struct Directorio(PathBuf);

    impl Directorio {
        fn nuevo(nombre: &str) -> Self {
            let ruta = std::env::temp_dir().join(format!("eje-latam-ciclo-{nombre}"));
            let _ = std::fs::remove_dir_all(&ruta);
            std::fs::create_dir_all(&ruta).expect("directorio de prueba");
            Self(ruta)
        }

        fn evidencia(&self) -> PathBuf {
            self.0.join("evidencia.alm")
        }
    }

    impl Drop for Directorio {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Ciclo con espia, y el buzon donde mirar lo que salio al cable.
    fn ciclo_con_espia(
        directorio: &Directorio,
        registro: RegistroEvidencia,
    ) -> (Ciclo<DespachoEspia>, Rc<RefCell<Vec<String>>>) {
        let buzon = Rc::new(RefCell::new(Vec::new()));
        let emisor = Emisor::nuevo(
            DespachoEspia {
                buzon: Rc::clone(&buzon),
            },
            "sensor-prueba",
        );

        (
            Ciclo::nuevo(
                directorio.evidencia(),
                PerfilSegmento::Ot,
                registro,
                Some(emisor),
            ),
            buzon,
        )
    }

    fn tramas(cuantas: usize) -> Vec<Observacion> {
        (0..cuantas)
            .map(|indice| Observacion {
                origen: [MAC, OTRA][indice & 1],
                protocolo: None,
                vlan: None,
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Lo que solo se ve ejecutando mas de una vuelta — PA-68
    // -----------------------------------------------------------------------

    #[test]
    fn el_almacen_recuerda_entre_vueltas_y_no_se_reinicia() {
        // Un demonio que olvida cada minuto es peor que un recorrido que recuerda
        // una vez: la ambiguedad pegajosa de RPT-010 §5 protege al carro de
        // telemedicina precisamente porque sobrevive al paso del tiempo.
        let directorio = Directorio::nuevo("memoria");
        let (mut ciclo, _) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::PrimerArranque;

        ciclo.vuelta(&estado, &tramas(2), 1_000, 1_000_000);
        let tras_una = ciclo.almacen().volatiles();

        ciclo.vuelta(&estado, &[], 2_000, 2_000_000);

        assert_eq!(
            ciclo.almacen().volatiles(),
            tras_una,
            "una vuelta sin tramas no puede borrar lo aprendido en la anterior"
        );
        assert!(tras_una > 0, "la primera vuelta debe haber observado algo");
    }

    #[test]
    fn las_alertas_anteriores_no_se_reemiten_en_cada_vuelta() {
        // El segundo defecto de la misma familia que el reloj congelado, y el
        // motivo por el que este modulo existe. `main.rs` consultaba SIEMPRE
        // `desde_asiento: 0`, asi que en modo continuo el SIEM del cliente
        // recibia el historial entero de alertas una vez por ciclo, para siempre.
        //
        // Correcto en un recorrido de una vuelta. Devastador en un demonio.
        let directorio = Directorio::nuevo("sin-reemision");

        let mut previo = RegistroEvidencia::nuevo();
        for indice in 0..3 {
            let _ = anotar_incontenible(
                &mut previo,
                1_000 + indice,
                &MAC,
                &Veredicto::Prohibida {
                    clase: ClaseExcluida::SoporteVital,
                },
            );
        }

        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, previo);
        let estado = EstadoArranque::PrimerArranque;

        for vuelta in 0..3 {
            let resultado = ciclo.vuelta(&estado, &tramas(2), 1_000 + vuelta, 1_000_000);
            assert!(
                resultado.anexadas.is_empty(),
                "la vuelta {vuelta} no anexo nada y no debe reportar alertas"
            );
        }

        let emitidos = buzon.borrow();

        // Se discrimina por el identificador de mensaje y no por `asiento=`: el
        // sello de PA-64 tambien lo lleva, y tiene todo el derecho a estar ahi.
        assert!(
            emitidos
                .iter()
                .all(|marco| !marco.contains("amenaza-incontenible")),
            "ninguna alerta del pasado puede volver al cable: {emitidos:?}"
        );

        // Y lo que si debe haber es exactamente un sello: el de la linea base,
        // con el extremo que ya estaba en el registro al arrancar. Comprobarlo
        // aqui impide que este filtro se relaje algun dia hasta dejar de mirar.
        assert_eq!(
            emitidos
                .iter()
                .filter(|marco| marco.contains("sello-de-evidencia"))
                .count(),
            1,
            "{emitidos:?}"
        );
        assert_eq!(emitidos.len(), 1, "y nada mas: {emitidos:?}");
    }

    #[test]
    fn una_condicion_estable_se_emite_una_sola_vez_en_muchas_vueltas() {
        // RPT-032 §3 estaba probado sobre `transiciones()` en aislamiento, nunca
        // a traves del ciclo. Es donde importa: la fatiga de alertas la produce
        // el bucle, no la funcion.
        let directorio = Directorio::nuevo("estable");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::PrimerArranque;

        ciclo.anotar_perdida();
        for vuelta in 0..5 {
            ciclo.vuelta(&estado, &tramas(1), 1_000 + vuelta, 1_000_000);
        }

        let perdidas = buzon
            .borrow()
            .iter()
            .filter(|marco| marco.contains("capturaConPerdida"))
            .count();

        assert_eq!(
            perdidas, 1,
            "cinco vueltas con la misma condicion son una noticia, no cinco"
        );
    }

    #[test]
    fn el_reloj_de_la_vuelta_llega_al_cable_y_no_se_queda_en_el_de_la_primera() {
        // El defecto de RPT-036 §3 en su forma observable sin inventario
        // firmado: lo que sale del ciclo debe llevar la hora de SU vuelta.
        let directorio = Directorio::nuevo("reloj");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::PrimerArranque;

        // Primera vuelta limpia: no hay transicion que emitir.
        ciclo.vuelta(&estado, &tramas(1), 1_000, 1_000_000_000);
        assert!(buzon.borrow().is_empty());

        // La condicion aparece en la segunda, con un reloj muy posterior.
        ciclo.anotar_perdida();
        ciclo.vuelta(&estado, &tramas(1), 2_000, 1_700_000_000_000);

        let emitido = buzon.borrow().first().cloned().expect("hubo transicion");
        assert!(
            emitido.contains(&crate::salida::marca_de_tiempo(1_700_000_000_000)),
            "el marco debe llevar la hora de la segunda vuelta: {emitido}"
        );
        assert!(
            !emitido.contains(&crate::salida::marca_de_tiempo(1_000_000_000)),
            "y no la de la primera: {emitido}"
        );
    }

    #[test]
    fn sin_alertas_no_se_escribe_el_disco_por_muchas_vueltas_que_den() {
        // RPT-034 §1.1. Si se escribiera en cada vuelta, un sensor tranquilo
        // reescribiria el fichero entero cada ciclo y el coste seria cuadratico
        // en la vida del proceso.
        let directorio = Directorio::nuevo("sin-escritura");
        let (mut ciclo, _) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::PrimerArranque;

        for vuelta in 0..10 {
            let resultado = ciclo.vuelta(&estado, &tramas(3), 1_000 + vuelta, 1_000_000);
            assert!(!resultado.persistido, "vuelta {vuelta}");
            assert!(!resultado.evidencia_en_riesgo());
        }

        assert!(
            !directorio.evidencia().exists(),
            "diez vueltas sin novedad no deben crear el fichero"
        );
    }

    #[test]
    fn el_registro_cargado_del_disco_continua_su_serie_a_traves_del_ciclo() {
        // Que la serie continue estaba probado sobre el registro; que el CICLO no
        // la reinicie, no. Es la diferencia entre la pieza y el bucle.
        let directorio = Directorio::nuevo("serie");

        let mut previo = RegistroEvidencia::nuevo();
        let _ = anotar_incontenible(
            &mut previo,
            1_000,
            &MAC,
            &Veredicto::Prohibida {
                clase: ClaseExcluida::SoporteVital,
            },
        );
        persistir(&directorio.evidencia(), &previo).expect("persiste");

        let CargaRegistro::Conforme(recuperado) =
            cargar_desde(&directorio.evidencia()).expect("no es fallo de disco")
        else {
            panic!("lo recien escrito debe verificar");
        };

        // `CargaRegistro::Conforme` lleva el registro en un `Box`: el registro
        // es grande y la variante viajaria inflando toda la enumeracion.
        let (mut ciclo, _) = ciclo_con_espia(&directorio, *recuperado);
        ciclo.vuelta(
            &EstadoArranque::PrimerArranque,
            &tramas(2),
            1_000,
            1_000_000,
        );

        assert_eq!(
            ciclo.registro().asientos().last().expect("hay uno").numero,
            1,
            "el ciclo no anexo nada y la serie sigue donde estaba"
        );
    }

    // -----------------------------------------------------------------------
    // Testigo externo del extremo — RPT-038, PA-64
    // -----------------------------------------------------------------------

    #[test]
    fn al_arrancar_sobre_un_registro_existente_su_extremo_sale_hacia_el_testigo() {
        // La linea base, y el motivo entero de PA-64. El ataque que el ancla no
        // ve es: parar el agente, recortar el registro, recalcular el ancla,
        // arrancar. El cotejo local dice `Conforme` porque el atacante lo hizo
        // bien. Lo unico que lo delata es que el colector tenia anotado un
        // asiento mas alto para esta maquina.
        let directorio = Directorio::nuevo("sello-linea-base");

        let mut previo = RegistroEvidencia::nuevo();
        for indice in 0..2 {
            let _ = anotar_incontenible(
                &mut previo,
                1_000 + indice,
                &MAC,
                &Veredicto::Prohibida {
                    clase: ClaseExcluida::SoporteVital,
                },
            );
        }
        let extremo = previo.extremo().hexadecimal();

        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, previo);
        let resultado = ciclo.vuelta(
            &EstadoArranque::PrimerArranque,
            &tramas(1),
            1_000,
            1_000_000,
        );

        assert!(resultado.sellado);

        let sellos: Vec<String> = buzon
            .borrow()
            .iter()
            .filter(|marco| marco.contains("sello="))
            .cloned()
            .collect();

        assert_eq!(sellos.len(), 1, "un arranque, un sello");
        assert!(
            sellos[0].contains(&format!("sello={extremo}")),
            "{sellos:?}"
        );
        assert!(sellos[0].contains("asiento=2"), "{sellos:?}");
    }

    #[test]
    fn un_extremo_que_no_cambia_no_se_vuelve_a_sellar() {
        // El extremo cambia exactamente cuando cambia el registro. Sellarlo en
        // cada vuelta convertiria el testigo en un latido, y un latido por
        // segundo durante un ano son los mismos treinta y dos bytes repetidos
        // treinta millones de veces en el almacenamiento del cliente.
        let directorio = Directorio::nuevo("sello-estable");

        let mut previo = RegistroEvidencia::nuevo();
        let _ = anotar_incontenible(
            &mut previo,
            1_000,
            &MAC,
            &Veredicto::Prohibida {
                clase: ClaseExcluida::SoporteVital,
            },
        );

        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, previo);
        let estado = EstadoArranque::PrimerArranque;

        for vuelta in 0..5 {
            ciclo.vuelta(&estado, &tramas(1), 1_000 + vuelta, 1_000_000);
        }

        let sellos = buzon
            .borrow()
            .iter()
            .filter(|marco| marco.contains("sello="))
            .count();

        assert_eq!(
            sellos, 1,
            "cinco vueltas sin novedad son un sello, no cinco"
        );
    }

    #[test]
    fn un_registro_vacio_no_sella_nada() {
        // Sin extremo no hay nada que atestiguar, por la misma razon por la que
        // `ancla_de` devuelve `None`: sellar el genesis haria indistinguible
        // «vacio» de «con todo borrado».
        let directorio = Directorio::nuevo("sello-vacio");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        let resultado = ciclo.vuelta(
            &EstadoArranque::PrimerArranque,
            &tramas(2),
            1_000,
            1_000_000,
        );

        assert!(!resultado.sellado);
        assert!(
            buzon.borrow().iter().all(|marco| !marco.contains("sello=")),
            "{:?}",
            buzon.borrow()
        );
    }

    #[test]
    fn el_estado_administrativo_se_declara_en_cada_vuelta_y_no_solo_en_la_primera() {
        // Las condiciones se derivan, no se guardan (RPT-019). Aqui se comprueba
        // que esa propiedad sobrevive al bucle: la vuelta cincuenta debe decir lo
        // mismo que la primera si nada cambio.
        let directorio = Directorio::nuevo("administrativo");
        let (mut ciclo, _) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::SinClaveAprovisionada;

        for vuelta in 0..3 {
            let resultado = ciclo.vuelta(&estado, &tramas(1), 1_000 + vuelta, 1_000_000);

            assert!(
                resultado.condiciones.accion_administrativa,
                "vuelta {vuelta}"
            );
            assert!(!resultado.condiciones.hay_manipulacion(), "vuelta {vuelta}");
        }
    }

    // -----------------------------------------------------------------------
    // Evidencia en riesgo — RPT-044, PA-69
    // -----------------------------------------------------------------------

    #[test]
    fn una_escritura_imposible_enciende_el_riesgo_y_se_reintenta_sin_anexar_nada() {
        // Fallo de escritura REAL: la ruta del registro se ocupa con un
        // directorio, asi que ningun renombrado atomico puede caer encima. El
        // codigo ve el mismo error que veria con el volumen lleno.
        let directorio = Directorio::nuevo("riesgo-encendido");
        std::fs::create_dir_all(directorio.evidencia()).expect("ocupar la ruta");

        let (mut ciclo, _) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        let (persistido, fallo) = ciclo.asegurar_durabilidad(true, 1_000);
        assert!(!persistido);
        assert!(
            fallo.is_some(),
            "el motivo del fallo debe llegar a quien llama"
        );

        // Y aqui lo que faltaba antes de PA-69: **sin anexar nada**, la vuelta
        // siguiente vuelve a intentarlo. Con la guarda vieja esto devolvia
        // `(false, None)` y el riesgo se quedaba para siempre.
        let (persistido, fallo) = ciclo.asegurar_durabilidad(false, 2_000);
        assert!(!persistido);
        assert!(
            fallo.is_some(),
            "un estado sucio debe reintentar aunque la vuelta no anexe"
        );
    }

    #[test]
    fn al_recuperar_queda_constancia_del_tramo_en_el_registro() {
        // La parte que una condicion no puede dar: la condicion se apaga sola y
        // un fallo de dos segundos no lo veria nadie. El asiento se anexa **al
        // recuperar**, cuando el disco funciona por definicion, y describe el
        // tramo entero.
        let directorio = Directorio::nuevo("constancia-del-tramo");
        std::fs::create_dir_all(directorio.evidencia()).expect("ocupar la ruta");

        let (mut ciclo, _) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        assert!(!ciclo.asegurar_durabilidad(true, 1_000).0);
        assert!(!ciclo.asegurar_durabilidad(false, 2_000).0);
        assert_eq!(
            ciclo.registro().longitud(),
            0,
            "durante el fallo NO se anexa nada: iria al fichero que no se puede escribir"
        );

        // El disco vuelve.
        std::fs::remove_dir(directorio.evidencia()).expect("liberar la ruta");

        let (persistido, fallo) = ciclo.asegurar_durabilidad(false, 3_000);
        assert!(persistido);
        assert!(fallo.is_none());

        let ultimo = ciclo
            .registro()
            .asientos()
            .last()
            .expect("la recuperacion deja constancia");

        assert_eq!(
            ultimo.clase,
            eje_almacen::ClaseEvento::PersistenciaRestablecida
        );
        assert!(
            ultimo.detalle.contains("2 vuelta"),
            "el asiento debe decir cuanto duro el tramo: {}",
            ultimo.detalle
        );

        // Y esa constancia esta en disco, no solo en memoria.
        let bytes = std::fs::read(directorio.evidencia()).expect("leer");
        let recuperado = eje_almacen::persistencia::analizar(&bytes).expect("verifica");
        assert_eq!(recuperado.longitud(), 1);
    }

    #[test]
    fn sin_fallo_de_escritura_no_se_declara_riesgo_ni_se_anexa_constancia() {
        let directorio = Directorio::nuevo("sin-riesgo");
        let (mut ciclo, _) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        let (persistido, fallo) = ciclo.asegurar_durabilidad(true, 1_000);
        assert!(persistido);
        assert!(fallo.is_none());
        assert_eq!(
            ciclo.registro().longitud(),
            0,
            "una escritura que funciona no deja constancia de nada"
        );

        // Sin anexado y sin riesgo, no se toca el disco.
        assert_eq!(ciclo.asegurar_durabilidad(false, 2_000), (false, None));
    }
}
