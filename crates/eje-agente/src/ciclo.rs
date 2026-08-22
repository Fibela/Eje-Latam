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

use crate::alertas::{
    EstadoConfiguracion, anotar_incontenible, condiciones, consultar, persistir, rotar_si_toca,
};
use crate::salida::{Despacho, Emisor, INTERVALO_LATIDO_MS, Latido};

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
    /// Que paso con el latido en esta vuelta. RPT-052, PA-104.
    ///
    /// No es un `bool` a proposito: ver [`Latido`]. Tres de sus cuatro estados
    /// producen el mismo silencio en el cable y significan cosas distintas.
    pub latido: Latido,
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
    /// Cada cuanto late el sensor, en milisegundos. RPT-057, PA-105.
    ///
    /// Arranca en [`INTERVALO_LATIDO_MS`] y se puede declarar. **Es provisional**:
    /// el intervalo viaja dentro del propio latido para que el colector no tenga
    /// que suponerlo, asi que dejarlo en manos de un argumento de linea de ordenes
    /// permite que quien controle el arranque del proceso alargue la ventana de
    /// silencio que la sala vigila. Sale a configuracion firmada en PA-79.
    intervalo_latido_ms: i64,
    /// El sensor no esta observando. RPT-047, PA-81.
    ///
    /// Vive en el ciclo y no se deriva del almacen porque el almacen solo sabe
    /// de lo que llego; que la captura no se pueda abrir es un hecho de fuera.
    captura_no_disponible: bool,
    /// Ninguna consola puede conectarse a este sensor. RPT-070, PA-125.
    ///
    /// Vive aqui por lo mismo que [`Self::captura_no_disponible`]: que el socket
    /// se pudiera abrir es un hecho de fuera del ciclo, y lo sabe quien lo
    /// intento.
    escucha_no_disponible: bool,
    /// En que estado esta la configuracion firmada. RPT-074, PA-79.
    ///
    /// Vive aqui por lo mismo que los dos de arriba: leerla y verificarla es un
    /// hecho de fuera del ciclo, y lo sabe quien lo intento.
    configuracion: EstadoConfiguracion,
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
            intervalo_latido_ms: INTERVALO_LATIDO_MS,
            captura_no_disponible: false,
            // Arranca en `false` y quien abre la escucha lo declara. No es un
            // supuesto optimista: `declarar_escucha` se llama siempre, tambien
            // cuando salio bien, por lo mismo que `declarar_captura`.
            escucha_no_disponible: false,
            // Arranca en `Ausente` y no en `Firmada`: el estado de partida tiene
            // que ser el que **no** afirma nada bueno. Si arrancara en `Firmada`,
            // un `main` que olvidara declararla dejaria a todos los sensores
            // diciendo que estan aprovisionados sin haberlo comprobado nadie.
            configuracion: EstadoConfiguracion::Ausente,
        }
    }

    /// Declara en que estado esta la configuracion firmada. RPT-074, PA-79.
    ///
    /// Se fija en cada vuelta, como las otras dos, aunque el fichero se lea una
    /// sola vez al arrancar: un estado que solo se fija al cambiar se queda
    /// pegado el dia que alguien olvide el camino de vuelta.
    pub const fn declarar_configuracion(&mut self, estado: EstadoConfiguracion) {
        self.configuracion = estado;
    }

    /// Declara si alguna consola puede conectarse. RPT-070, PA-125.
    ///
    /// # Por que se declara y no se deduce
    ///
    /// El ciclo no abre el socket: lo abre `main` una vez, fuera del bucle
    /// (PA-66). Desde dentro, un sensor sin escucha y uno al que nadie ha
    /// preguntado son identicos —cero consultas atendidas—, que es la misma
    /// indistinguibilidad que obligo a declarar la captura.
    ///
    /// Se fija aunque no cambie, por la misma razon de siempre: un estado que
    /// solo se fija al cambiar se queda pegado si alguien olvida el camino de
    /// vuelta.
    pub const fn declarar_escucha(&mut self, disponible: bool) {
        self.escucha_no_disponible = !disponible;
    }

    /// Fija cada cuanto late el sensor. RPT-057, PA-105.
    ///
    /// Se llama siempre, tambien cuando el valor es el de por omision, por lo
    /// mismo que [`Self::declarar_captura`]: un ajuste que solo se aplica cuando
    /// cambia es un ajuste que alguien creera aplicado sin estarlo.
    ///
    /// Un intervalo que no sea positivo se ignora: cero convertiria cada vuelta en
    /// un latido y llenaria el colector, y un negativo haria lo mismo por otro
    /// camino. No se corrige a un valor cercano —eso seria obedecer a medias— se
    /// conserva el vigente.
    pub const fn declarar_intervalo_latido(&mut self, milisegundos: i64) {
        if milisegundos > 0 {
            self.intervalo_latido_ms = milisegundos;
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
            )
            .sucesos;
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
        // `sin_colector` sale de si hay emisor, que es la misma fuente de la que
        // sale `Latido::SinColector` mas abajo. Derivarlo de un interruptor
        // aparte permitiria que las dos se contradijeran: el tablero diria que
        // hay colector y el latido que no.
        let base = condiciones(
            estado,
            &self.almacen,
            &self.registro,
            self.captura_no_disponible,
            self.emisor.is_none(),
            self.escucha_no_disponible,
            self.configuracion,
        );

        // `evidenciaEnRiesgo` **si** es emisible, y hasta aqui salia siempre
        // apagada: `condiciones` la devuelve en falso por construccion y el valor
        // real solo se ponia al construir el `Resultado`, ya despues de emitir.
        // La transicion de PA-69 se calculaba sobre un campo que no cambiaba
        // nunca, asi que la perdida de evidencia jamas llego al SIEM.
        //
        // Se completa aqui porque aqui ya se sabe: `asegurar_durabilidad` corrio
        // mas arriba en esta misma vuelta. `salidaNoDisponible` es el unico campo
        // que sigue pendiente, y ese no viaja por syslog (RPT-032 §4).
        let vigentes = Condiciones {
            evidencia_en_riesgo: self.riesgo_desde.is_some(),
            ..base
        };

        let mut salida_bien = self
            .emisor
            .as_mut()
            .is_none_or(|emisor| emisor.emitir(&anexadas, &vigentes, ahora_ms));

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

        // PA-104. El latido va DESPUES del sello y no sustituye a ninguno de los
        // dos envios anteriores.
        //
        // `sellar` calla cuando el extremo no cambia —correcto, RPT-032 §3— y en
        // un sensor tranquilo el extremo **no cambia nunca**. Ese silencio es
        // precisamente el caso que la sala no sabe leer: un sensor en calma y uno
        // desenchufado producen el mismo dato, que es ninguno (RPT-052 §1).
        //
        // Se late tambien con el registro vacio, donde `sellar` no emite: un
        // `asiento=0` con el genesis dice «vivo y todavia sin evidencia», y eso
        // es una afirmacion, no un hueco.
        //
        // Y se late con el sensor ciego. Un latido que se apaga al degradarse
        // borraria la unica diferencia que importa en la sala: un sensor que se
        // apago y uno que dejo de ver son dos llamadas distintas (RPT-052 §4).
        let latido = match &mut self.emisor {
            Some(emisor) => emisor.latir(
                ultimo,
                &self.registro.extremo().hexadecimal(),
                &vigentes,
                self.intervalo_latido_ms,
                ahora_ms,
                ahora_ms,
            ),
            None => Latido::SinColector,
        };

        // Solo el fallo cuenta como salida caida. `NoTocaba` es funcionamiento
        // normal y `SinColector` es una decision de despliegue: tomar cualquiera
        // de los dos por averia encenderia `salidaNoDisponible` de forma
        // permanente en todo agente sin colector.
        if latido == Latido::NoSePudo {
            salida_bien = false;
        }

        Resultado {
            condiciones: Condiciones {
                salida_no_disponible: !salida_bien,
                ..vigentes
            },
            con_marcado,
            contenibles,
            escalados,
            anexadas,
            persistido,
            fallo_persistencia,
            sellado,
            latido,
            perdidas,
            rotado,
        }
    }
}

#[cfg(test)]
mod pruebas {
    // Mismo encabezado que el resto de modulos de prueba del workspace.
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use std::cell::{Cell, RefCell};
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

    /// Despacho que falla mientras alguien mantenga `roto` a cierto.
    ///
    /// El colector caido no es un caso raro: es el caso para el que existe
    /// `salidaNoDisponible`. Un espia que siempre acepta no puede ejercitarlo.
    struct DespachoCaprichoso {
        buzon: Rc<RefCell<Vec<String>>>,
        roto: Rc<Cell<bool>>,
    }

    impl Despacho for DespachoCaprichoso {
        fn enviar(&mut self, marco: &[u8]) -> Result<(), ErrorSalida> {
            if self.roto.get() {
                return Err(ErrorSalida::NoDisponible {
                    detalle: "colector caido en la prueba".to_owned(),
                });
            }
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
            "eth-prueba",
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

    /// Banco de pruebas con colector averiable: el ciclo, lo que salio al cable
    /// y el interruptor que tira el colector.
    type BancoCaprichoso = (
        Ciclo<DespachoCaprichoso>,
        Rc<RefCell<Vec<String>>>,
        Rc<Cell<bool>>,
    );

    /// Ciclo cuyo colector se puede tirar y levantar a voluntad.
    fn ciclo_con_colector_caprichoso(directorio: &Directorio) -> BancoCaprichoso {
        let buzon = Rc::new(RefCell::new(Vec::new()));
        let roto = Rc::new(Cell::new(false));
        let emisor = Emisor::nuevo(
            DespachoCaprichoso {
                buzon: Rc::clone(&buzon),
                roto: Rc::clone(&roto),
            },
            "sensor-prueba",
            "eth-prueba",
        );

        (
            Ciclo::nuevo(
                directorio.evidencia(),
                PerfilSegmento::Ot,
                RegistroEvidencia::nuevo(),
                Some(emisor),
            ),
            buzon,
            roto,
        )
    }

    /// Los latidos que hay en el buzon, en orden.
    fn latidos(buzon: &Rc<RefCell<Vec<String>>>) -> Vec<String> {
        buzon
            .borrow()
            .iter()
            .filter(|marco| marco.contains("latido-de-sensor"))
            .cloned()
            .collect()
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
        // Se aisla la configuracion: lo que se cuenta aqui son marcos emitidos, y
        // una transicion de mas los descuadra sin decir nada del defecto que esta
        // prueba vigila (RPT-074, PA-79).
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
        ciclo.declarar_configuracion(EstadoConfiguracion::Firmada);
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

        // Y un latido, el de la primera vuelta: las tres comparten `ahora_ms`,
        // asi que el intervalo no vence. Ver `late_en_calma_...` mas abajo.
        assert_eq!(
            emitidos
                .iter()
                .filter(|marco| marco.contains("latido-de-sensor"))
                .count(),
            1,
            "{emitidos:?}"
        );
        assert_eq!(emitidos.len(), 2, "y nada mas: {emitidos:?}");
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

        // `condicion=` y no solo el nombre: el latido lleva las condiciones
        // vigentes en `condiciones=capturaConPerdida`, y contarlo aqui haria
        // fallar esta prueba por un motivo que no tiene que ver con la fatiga de
        // alertas. Son dos mensajes distintos y se discriminan por el campo.
        let perdidas = buzon
            .borrow()
            .iter()
            .filter(|marco| marco.contains("condicion=capturaConPerdida"))
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

        // Se aisla la configuracion: esta prueba mira el reloj, y una transicion
        // de `configuracionSinFirmar` en la primera vuelta romperia la afirmacion
        // de que no hay ninguna todavia.
        ciclo.declarar_configuracion(EstadoConfiguracion::Firmada);

        // Primera vuelta limpia: no hay transicion que emitir. Si que hay latido
        // —PA-104 lo emite tambien en calma—, asi que se mira lo que interesa y
        // no el buzon entero.
        ciclo.vuelta(&estado, &tramas(1), 1_000, 1_000_000_000);
        assert!(
            buzon
                .borrow()
                .iter()
                .all(|marco| !marco.contains("condicion=")),
            "{:?}",
            buzon.borrow()
        );

        // La condicion aparece en la segunda, con un reloj muy posterior.
        ciclo.anotar_perdida();
        ciclo.vuelta(&estado, &tramas(1), 2_000, 1_700_000_000_000);

        let emitido = buzon
            .borrow()
            .iter()
            .find(|marco| marco.contains("condicion="))
            .cloned()
            .expect("hubo transicion");
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

        // Por el identificador del mensaje, no por `sello=`: el latido de PA-104
        // lleva el mismo par (asiento, extremo) en su linea y contarlo aqui
        // convertiria «un arranque, un sello» en dos.
        let sellos: Vec<String> = buzon
            .borrow()
            .iter()
            .filter(|marco| marco.contains("sello-de-evidencia"))
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
            .filter(|marco| marco.contains("sello-de-evidencia"))
            .count();

        assert_eq!(
            sellos, 1,
            "cinco vueltas sin novedad son un sello, no cinco"
        );

        // El latido es lo contrario y por eso vive aparte: si algun dia alguien
        // hace que `sellar` lata, esta prueba se pone roja y la de mas abajo
        // sigue verde, que es justo la senal que hace falta.
        assert_eq!(
            buzon
                .borrow()
                .iter()
                .filter(|marco| marco.contains("latido-de-sensor"))
                .count(),
            1,
            "con el reloj parado el intervalo no vence: un latido"
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
            buzon
                .borrow()
                .iter()
                .all(|marco| !marco.contains("sello-de-evidencia")),
            "{:?}",
            buzon.borrow()
        );

        // Pero el latido SI sale, y es la diferencia entera de PA-104: un sensor
        // recien instalado sobre un registro vacio esta vivo, y callarse hasta la
        // primera alerta lo hace indistinguible de uno que nunca arranco.
        assert_eq!(resultado.latido, Latido::Emitido);
        assert!(
            buzon
                .borrow()
                .iter()
                .any(|marco| marco.contains("latido-de-sensor") && marco.contains("asiento=0")),
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

    // -----------------------------------------------------------------------
    // El latido — RPT-052, PA-104
    // -----------------------------------------------------------------------

    #[test]
    fn late_en_calma_aunque_no_ocurra_absolutamente_nada() {
        // El caso entero de PA-104. Sin tramas, sin alertas y sin transiciones,
        // el agente no emitia una sola linea: desde la sala, un sensor tranquilo
        // y uno desenchufado producen el mismo dato, que es ninguno.
        let directorio = Directorio::nuevo("latido-en-calma");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        // Calma incluye tener configuracion firmada (RPT-074, PA-79). Sin
        // declararlo, el ciclo arranca en `Ausente` —que es lo correcto— y esta
        // prueba mediria otra cosa.
        ciclo.declarar_configuracion(EstadoConfiguracion::Firmada);

        let resultado = ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000, 1_000_000);

        assert_eq!(resultado.latido, Latido::Emitido);

        let latidos = latidos(&buzon);
        assert_eq!(latidos.len(), 1, "{latidos:?}");

        // «Ninguna» es una afirmacion, no un hueco: dice que se miraron las once
        // condiciones emisibles y no habia ninguna activa. Omitir la lista
        // dejaria al colector sin saber si se comprobo.
        assert!(latidos[0].contains("condiciones=ninguna"), "{latidos:?}");

        // Y el intervalo viaja dentro (RPT-052 §5): el colector no tiene que
        // suponer cuanto esperar antes de dar la ausencia por cierta.
        assert!(
            latidos[0].contains(&format!("intervaloMs={INTERVALO_LATIDO_MS}")),
            "{latidos:?}"
        );
    }

    #[test]
    fn no_late_dos_veces_dentro_del_mismo_intervalo() {
        // Un latido por vuelta seria un latido por segundo: el mismo error que
        // `un_extremo_que_no_cambia_no_se_vuelve_a_sellar` evita en el sello, y
        // aqui costaria treinta millones de lineas al ano en el cliente.
        let directorio = Directorio::nuevo("latido-intervalo");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::PrimerArranque;
        let inicio = 1_700_000_000_000_i64;

        assert_eq!(
            ciclo.vuelta(&estado, &[], 1_000, inicio).latido,
            Latido::Emitido,
            "el primero sale sin esperar: la ventana de silencio al arrancar es \
             justo cuando mas probable es que algo este mal configurado"
        );

        // Un milisegundo antes de cumplirse el intervalo todavia no toca. El
        // borde exacto se prueba porque un `<=` mal puesto aqui no lo nota nadie.
        assert_eq!(
            ciclo
                .vuelta(&estado, &[], 1_030, inicio + INTERVALO_LATIDO_MS - 1)
                .latido,
            Latido::NoTocaba
        );

        assert_eq!(
            ciclo
                .vuelta(&estado, &[], 1_060, inicio + INTERVALO_LATIDO_MS)
                .latido,
            Latido::Emitido
        );

        assert_eq!(latidos(&buzon).len(), 2);
    }

    #[test]
    fn un_reloj_que_retrocede_no_deja_al_sensor_mudo() {
        // El reloj que llega al ciclo es de pared, no monotono. Un `ntpd` que
        // corrige hacia atras da un transcurrido negativo, y leerlo como «acabo
        // de latir» callaria al agente todo lo que dure el salto —horas, si el
        // salto es de horas— mientras la sala lo da por muerto.
        let directorio = Directorio::nuevo("latido-reloj-atras");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::PrimerArranque;
        let inicio = 1_700_000_000_000_i64;

        ciclo.vuelta(&estado, &[], 1_000, inicio);

        assert_eq!(
            ciclo.vuelta(&estado, &[], 1_030, inicio - 3_600_000).latido,
            Latido::Emitido,
            "ante la duda se late de mas: un latido sobrante es ruido, uno que \
             falta es una llamada de madrugada"
        );

        assert_eq!(latidos(&buzon).len(), 2);
    }

    #[test]
    fn el_sensor_ciego_sigue_latiendo_y_lo_dice_en_el_latido() {
        // La prueba que de verdad importa. Un latido que se apaga al degradarse
        // borra la unica diferencia que la sala necesita: un sensor que se apago
        // y uno que dejo de ver son dos llamadas distintas (RPT-052 §4).
        let directorio = Directorio::nuevo("latido-ciego");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        ciclo.declarar_captura(false);
        let resultado = ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000, 1_000_000);

        assert_eq!(resultado.latido, Latido::Emitido);

        let latidos = latidos(&buzon);
        assert!(
            latidos[0].contains("condiciones=capturaNoDisponible"),
            "el estado viaja con el latido o la sala tiene que preguntarlo por un \
             camino que no tiene: {latidos:?}"
        );
    }

    /// El sensor sin escucha local llega a la sala por el canal que le queda.
    ///
    /// RPT-070, PA-125. Gemela de la de arriba y por el mismo motivo: aquella
    /// cubre el sensor que dejo de ver, esta el que dejo de poder ser preguntado.
    ///
    /// La diferencia esta en quien se entera. Un sensor ciego lo cuenta por los
    /// dos caminos —la consola y la sala—; uno sin escucha **solo por este**,
    /// porque el otro camino es justamente el que falta.
    #[test]
    fn el_sensor_sin_escucha_lo_dice_en_el_latido() {
        let directorio = Directorio::nuevo("latido-sin-escucha");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        ciclo.declarar_escucha(false);
        let resultado = ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000, 1_000_000);

        assert_eq!(resultado.latido, Latido::Emitido);

        let latidos = latidos(&buzon);
        assert!(
            latidos[0].contains("escuchaNoDisponible"),
            "sin esto, un sensor vivo e inalcanzable no existe para nadie: {latidos:?}"
        );
    }

    /// Y declarar que la escucha volvio la apaga.
    ///
    /// Un estado que solo se fija al degradarse se queda pegado, y un sensor que
    /// se declara incomunicado para siempre ensena a ignorar la condicion.
    #[test]
    fn la_escucha_recuperada_deja_de_declararse_caida() {
        let directorio = Directorio::nuevo("latido-escucha-vuelve");
        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        ciclo.declarar_escucha(false);
        ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000, 1_000_000);

        ciclo.declarar_escucha(true);
        ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000_000, 2_000_000);

        let latidos = latidos(&buzon);
        let ultimo = latidos.last().expect("hubo dos latidos");
        assert!(
            !ultimo.contains("escuchaNoDisponible"),
            "la condicion se quedo pegada tras recuperarse la escucha: {ultimo}"
        );
    }

    /// Los tres estados de la configuracion producen tres condiciones distintas.
    ///
    /// RPT-074, PA-79. Es la prueba de que los dos booleanos **nunca son ambos
    /// ciertos**: se derivan de un solo dato, y por eso el estado imposible no
    /// existe. Con dos interruptores independientes habria que confiar en que
    /// nadie encienda los dos.
    #[test]
    fn cada_estado_de_configuracion_enciende_exactamente_una_condicion() {
        let directorio = Directorio::nuevo("configuracion-tres-estados");
        let (mut ciclo, _buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        for (estado, sin_firmar, no_verifica) in [
            (EstadoConfiguracion::Firmada, false, false),
            (EstadoConfiguracion::Ausente, true, false),
            (EstadoConfiguracion::NoVerifica, false, true),
        ] {
            ciclo.declarar_configuracion(estado);
            let resultado = ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000, 1_000_000);

            assert_eq!(
                resultado.condiciones.configuracion_sin_firmar, sin_firmar,
                "{estado:?} no produjo la condicion esperada"
            );
            assert_eq!(
                resultado.condiciones.configuracion_no_verifica, no_verifica,
                "{estado:?} no produjo la condicion esperada"
            );
        }
    }

    /// Y el ciclo arranca declarando lo que **no** afirma nada bueno.
    ///
    /// Si arrancara en `Firmada`, un `main` que olvidara declararla dejaria a
    /// todos los sensores diciendo que estan aprovisionados sin que nadie lo
    /// hubiera comprobado. Es la misma eleccion que `escucha_no_disponible`.
    #[test]
    fn sin_declarar_nada_el_ciclo_no_finge_estar_aprovisionado() {
        let directorio = Directorio::nuevo("configuracion-por-omision");
        let (mut ciclo, _buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());

        let resultado = ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000, 1_000_000);

        assert!(
            resultado.condiciones.configuracion_sin_firmar,
            "el estado de partida no puede afirmar que hay configuracion firmada"
        );
    }

    #[test]
    fn un_latido_que_no_sale_no_marca_el_instante_y_se_reintenta() {
        // Si el fallo marcara el instante, un colector caido durante el latido
        // compraria un intervalo entero de silencio **adicional** a la caida. El
        // fallo no se acumula: la vuelta siguiente lo vuelve a intentar.
        let directorio = Directorio::nuevo("latido-reintento");
        let (mut ciclo, buzon, roto) = ciclo_con_colector_caprichoso(&directorio);
        let estado = EstadoArranque::PrimerArranque;

        roto.set(true);
        let caido = ciclo.vuelta(&estado, &[], 1_000, 1_000_000);
        assert_eq!(caido.latido, Latido::NoSePudo);
        assert!(
            caido.condiciones.salida_no_disponible,
            "un latido que no sale es la salida caida, y eso si viaja por IPC"
        );
        assert!(latidos(&buzon).is_empty());

        // El colector vuelve. **Sin adelantar el reloj**: si el instante se
        // hubiera marcado en el fallo, esta vuelta diria `NoTocaba` y el sensor
        // seguiria mudo un minuto mas.
        roto.set(false);
        let vuelto = ciclo.vuelta(&estado, &[], 1_030, 1_000_000);
        assert_eq!(vuelto.latido, Latido::Emitido);
        assert!(!vuelto.condiciones.salida_no_disponible);
        assert_eq!(latidos(&buzon).len(), 1);
    }

    #[test]
    fn sin_colector_no_se_late_y_el_agente_lo_declara() {
        // `SinColector` no es `NoTocaba` ni `NoSePudo`. Tomarlo por averia
        // encenderia `salidaNoDisponible` para siempre en todo agente sin
        // colector; tomarlo por normalidad ocultaria que **ese sensor no esta
        // cubierto por PA-105** y que nadie fuera notara si se apaga.
        let directorio = Directorio::nuevo("latido-sin-colector");
        let mut ciclo = Ciclo::<DespachoEspia>::nuevo(
            directorio.evidencia(),
            PerfilSegmento::Ot,
            RegistroEvidencia::nuevo(),
            None,
        );

        let resultado = ciclo.vuelta(&EstadoArranque::PrimerArranque, &[], 1_000, 1_000_000);

        assert_eq!(resultado.latido, Latido::SinColector);
        assert!(!resultado.condiciones.salida_no_disponible);
    }

    #[test]
    fn la_perdida_de_evidencia_llega_al_siem_y_no_solo_al_panel() {
        // PA-69 tenia su condicion, su prueba y su canal, y aun asi **nunca
        // salio**: `condiciones()` devuelve `evidenciaEnRiesgo` apagada por
        // construccion y el valor real se ponia al construir el `Resultado`, ya
        // despues de emitir. La transicion se calculaba sobre un campo que no
        // cambiaba nunca.
        //
        // El decimo mecanismo correcto que nadie llamaba.
        let directorio = Directorio::nuevo("riesgo-al-siem");
        std::fs::create_dir_all(directorio.evidencia()).expect("ocupar la ruta");

        let (mut ciclo, buzon) = ciclo_con_espia(&directorio, RegistroEvidencia::nuevo());
        let estado = EstadoArranque::PrimerArranque;

        // La escritura falla de verdad: la ruta esta ocupada por un directorio.
        assert!(!ciclo.asegurar_durabilidad(true, 1_000).0);
        let resultado = ciclo.vuelta(&estado, &[], 1_000, 1_000_000);

        assert!(resultado.condiciones.evidencia_en_riesgo);
        assert!(
            buzon
                .borrow()
                .iter()
                .any(|marco| marco.contains("condicion=evidenciaEnRiesgo")),
            "{:?}",
            buzon.borrow()
        );

        // Y tambien va en el latido, que es lo que ve una sala que solo lee del
        // colector (RPT-051 §2C).
        let latidos = latidos(&buzon);
        assert!(latidos[0].contains("evidenciaEnRiesgo"), "{latidos:?}");
    }
}
