//! # Guardian de Confianza Cero — AGT-01
//!
//! Inspeccion **pasiva** de trafico L2/L3 y contencion de nodos IoT/OT.
//!
//! ## Modelo de despliegue: Sensor Adyacente
//!
//! El agente **no se instala** en PLC, camaras ni bombas de infusion. Opera como
//! sensor que recibe copia del trafico via puerto SPAN, TAP pasivo, o desde el
//! gateway del segmento (RPT-002 §5, AGT-01).
//!
//! ## Terminologia
//!
//! El termino correcto es **Inspeccion Pasiva**. "Infeccion Pasiva" fue un error
//! del corpus original, retirado en RPT-002 §2.1.

#![forbid(unsafe_code)]

pub mod almacen;
pub mod arranque;
pub mod clasificacion;
pub mod clave;
pub mod configuracion;
pub mod disco;
pub mod formato;
pub mod inventario;
pub mod observacion;
pub mod proveedores;
pub mod revocacion;
pub mod vlan;

use clasificacion::{Clasificacion, MotivoAmbiguedad};
use thiserror::Error;

/// Errores del guardian.
#[derive(Debug, Error)]
pub enum ErrorGuardian {
    /// La captura no pudo iniciarse por falta de privilegios.
    #[error("privilegios insuficientes para captura en {interfaz}")]
    PrivilegiosInsuficientes {
        /// Interfaz sobre la que se intento capturar.
        interfaz: String,
    },

    /// La trama recibida no pudo analizarse.
    #[error("trama malformada en desplazamiento {desplazamiento}")]
    TramaMalformada {
        /// Desplazamiento en bytes donde fallo el analisis.
        desplazamiento: usize,
    },

    /// Se rechazo una orden de contencion originada en una simulacion.
    ///
    /// Ver [`OrigenEvento`] y RPT-003 §8.1.
    #[error("orden de contencion rechazada: origen de simulacion")]
    ContencionDeSimulacionRechazada,
}

/// Ruta de captura de paquetes disponible en la plataforma.
///
/// `AfPacket` es la ruta de **referencia** en Linux. `Ebpf` es una optimizacion
/// disponible solo en kernels modernos: los entornos OT operan con frecuencia
/// sobre kernels 3.10–4.x donde no existe (RPT-003 §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RutaCaptura {
    /// Linux — `AF_PACKET` con `PACKET_MMAP`. Ruta de referencia.
    AfPacket,
    /// Linux — eBPF/XDP. Optimizacion, requiere kernel moderno.
    Ebpf,
    /// Windows — Npcap. Requiere licencia OEM para redistribuir.
    Npcap,
    /// macOS — dispositivo BPF.
    Bpf,
}

/// Perfil operativo del segmento de red vigilado.
///
/// El perfil [`PerfilSegmento::Ot`] impone restricciones que no son opcionales:
/// descubrimiento pasivo y ausencia de trafico emitido por el agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfilSegmento {
    /// Red corporativa de proposito general.
    Corporativo,
    /// Red industrial u hospitalaria. La difusion activa puede degradar PLC antiguos.
    Ot,
}

impl PerfilSegmento {
    /// Indica si el agente puede emitir trafico de descubrimiento en el segmento.
    ///
    /// En perfil [`PerfilSegmento::Ot`] el descubrimiento es siempre pasivo
    /// (RPT-002 §9.2).
    #[must_use]
    pub const fn permite_descubrimiento_activo(self) -> bool {
        matches!(self, Self::Corporativo)
    }
}

/// Origen de un evento que llega al motor de respuesta.
///
/// # Requisito de seguridad de vida
///
/// `SIM-01` y la ruta de contencion residen en dominios de capacidad separados:
/// el simulador **no posee** la capacidad de invocar contencion (RPT-003 §8.1).
/// Este tipo es la segunda capa de defensa, no la primera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrigenEvento {
    /// Trafico real observado en el segmento.
    Produccion,
    /// Inyeccion de simulacro, marcada y firmada.
    Simulacion,
}

impl OrigenEvento {
    /// Indica si el evento habilita ejecutar contencion.
    ///
    /// El rechazo es la ruta por defecto: ante marca ausente, ilegible o invalida,
    /// el motor no actua.
    #[must_use]
    pub const fn habilita_contencion(self) -> bool {
        matches!(self, Self::Produccion)
    }
}

/// Mecanismo autorizado de contencion de un nodo.
///
/// # Prohibicion
///
/// La suplantacion ARP queda **terminantemente prohibida** para contencion en
/// redes de produccion (RPT-002 §5, AGT-01). Es la misma tecnica que un ataque de
/// intermediario y en OT puede provocar un incidente de seguridad fisica. Por eso
/// no existe una variante de este enum que la represente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MecanismoContencion {
    /// Apagado o cuarentena de puerto de switch via SNMPv3 en modo `authPriv`.
    ///
    /// SNMPv1 y v2c quedan prohibidos: las cadenas de comunidad viajan sin cifrar
    /// (RPT-003 §6.4).
    SnmpV3,
    /// Apagado o cuarentena de puerto de switch via NETCONF sobre SSH.
    Netconf,
    /// Cambio de autorizacion 802.1X para reasignar a VLAN de cuarentena.
    RadiusCoa,
    /// Reglas de firewall local, cuando el nodo ejecuta el agente.
    FirewallLocal,
}

impl MecanismoContencion {
    /// Todos los mecanismos autorizados.
    pub const TODOS: [Self; 4] = [
        Self::SnmpV3,
        Self::Netconf,
        Self::RadiusCoa,
        Self::FirewallLocal,
    ];

    /// Identificador estable, tal como figura en `contrato-contencion.toml`.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::SnmpV3 => "SnmpV3",
            Self::Netconf => "Netconf",
            Self::RadiusCoa => "RadiusCoa",
            Self::FirewallLocal => "FirewallLocal",
        }
    }
}

/// Resultado de una contencion. RPT-008 §4.1.
///
/// # Por que son tres y no dos
///
/// [`EstadoContencion::Desconocido`] es `ComprobacionImposible` de RPT-006 §4
/// aplicado a la red, y aqui es el estado **dominante**, no el excepcional:
/// plazo agotado, sesion caida entre el envio y la confirmacion, escritura
/// aceptada pero relectura muda, aplicacion parcial en un apilamiento.
///
/// Un puerto que se cree aislado y no lo esta es peor que uno que se sabe no
/// aislado, porque el segundo escala a un humano y el primero no. **Prohibido
/// colapsarlo** en ninguno de los otros dos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoContencion {
    /// La accion se aplico **y** una relectura independiente lo confirma.
    ///
    /// Sin relectura este estado seria una declaracion de intencion con aspecto
    /// de hecho observado.
    Contenido,
    /// El equipo rechazo la accion y devolvio un motivo interpretable.
    Rechazada,
    /// Se emitio la accion y no se pudo determinar si surtio efecto.
    Desconocido,
}

impl EstadoContencion {
    /// Nombre estable, tal como figura en `contrato-contencion.toml`.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::Contenido => "Contenido",
            Self::Rechazada => "ContencionRechazada",
            Self::Desconocido => "EstadoDesconocido",
        }
    }

    /// Indica si el estado debe escalarse a un operador humano.
    ///
    /// `Desconocido` escala igual que `Rechazada`: no saber si el puerto quedo
    /// aislado exige intervencion tanto como saber que no lo quedo.
    #[must_use]
    pub const fn escala_a_humano(self) -> bool {
        !matches!(self, Self::Contenido)
    }
}

/// Clase de dispositivo excluida de toda contencion. RPT-008 §4.5.
///
/// Se evalua **antes** que el perfil y que cualquier politica. No es una
/// preferencia configurable: es un limite del producto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaseExcluida {
    /// Aislar un dispositivo de soporte vital es un evento clinico, no de red.
    SoporteVital,
    /// Paro de emergencia, cortinas opticas, enclavamientos. Aislarlos puede
    /// **provocar** la condicion insegura que se pretendia evitar.
    SeguridadFuncional,
    /// Aislar el camino por el que se administra el equipo hace la accion
    /// irreversible en remoto.
    CaminoDeGestion,
}

impl ClaseExcluida {
    /// Todas las clases excluidas.
    pub const TODAS: [Self; 3] = [
        Self::SoporteVital,
        Self::SeguridadFuncional,
        Self::CaminoDeGestion,
    ];

    /// Identificador estable, tal como figura en `contrato-contencion.toml`.
    #[must_use]
    pub const fn identificador(self) -> &'static str {
        match self {
            Self::SoporteVital => "soporte-vital",
            Self::SeguridadFuncional => "seguridad-funcional",
            Self::CaminoDeGestion => "camino-de-gestion",
        }
    }
}

/// Veredicto previo a emitir una contencion. RPT-008 §5, RPT-009.
///
/// # Dos formas distintas de decir que no
///
/// [`Veredicto::Prohibida`] es permanente y **nadie** la levanta: el dispositivo
/// *es* de una clase excluida. [`Veredicto::RequiereAprobacion`] escala a un
/// humano que puede decidir proceder: la evidencia no basta.
///
/// Confundirlas seria un defecto en cualquiera de las dos direcciones. Tratar
/// una ambiguedad como prohibicion permanente dejaria un dispositivo
/// incontenible para siempre por un falso positivo; tratar una prohibicion como
/// ambiguedad permitiria aislar una bomba de infusion con un clic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Veredicto {
    /// Puede ejecutarse sin intervencion.
    Ejecutar,
    /// Debe presentarse a un operador antes de escribir nada.
    ///
    /// El ensayo recorre precondiciones y resolucion de objetivo, y se detiene
    /// justo antes de la escritura.
    RequiereAprobacion {
        /// Por que se escala. `None` cuando lo impone el perfil y no la evidencia.
        motivo: Option<MotivoAmbiguedad>,
    },
    /// No puede ejecutarse por ninguna via.
    Prohibida {
        /// Clase que motiva la prohibicion.
        clase: ClaseExcluida,
    },
}

impl Veredicto {
    /// Indica si el veredicto obliga a notificar a un operador.
    ///
    /// Todo lo que no sea [`Veredicto::Ejecutar`] escala. Un bloqueo silencioso
    /// convierte la lista de exclusion en una via de evasion comoda: al atacante
    /// le bastaria parecer un equipo critico para que su actividad se archivara
    /// sin que nadie la viera.
    #[must_use]
    pub const fn exige_alerta(&self) -> bool {
        !matches!(self, Self::Ejecutar)
    }

    /// Indica si se detecto una amenaza sobre un dispositivo **incontenible**.
    ///
    /// Es la condicion mas urgente que este producto puede comunicar, y no puede
    /// tratarse como una alerta ordinaria: no existe accion automatica posible,
    /// asi que la unica respuesta es humana e inmediata. Aislar la bomba no es
    /// una opcion; aislar lo que la rodea, avisar a ingenieria clinica o a
    /// planta, si.
    #[must_use]
    pub const fn es_amenaza_incontenible(&self) -> bool {
        matches!(self, Self::Prohibida { .. })
    }
}

impl PerfilSegmento {
    /// Indica si el perfil admite contencion automatica sin aprobacion humana.
    ///
    /// El perfil OT **nunca** la admite. IEC 62443 ordena las prioridades de un
    /// sistema de automatizacion industrial al reves que TI: disponibilidad y
    /// seguridad fisica por encima de confidencialidad. Una contencion
    /// automatica que detiene una linea es, en ese marco, el incidente — no la
    /// respuesta al incidente.
    #[must_use]
    pub const fn permite_respuesta_automatica(self) -> bool {
        matches!(self, Self::Corporativo)
    }
}

/// Decide si una orden puede ejecutarse, y como. RPT-008 §5, RPT-009.
///
/// # Orden de evaluacion
///
/// La clasificacion se comprueba **antes** que el perfil, porque ninguna
/// aprobacion humana levanta una exclusion permanente. Si el perfil se
/// comprobara primero, un segmento corporativo ejecutaria contencion sobre un
/// dispositivo de soporte vital.
///
/// Ninguna combinacion de entradas produce [`Veredicto::Ejecutar`] partiendo de
/// una clasificacion que no sea [`Clasificacion::Clasificado`] con `clase: None`.
/// Esa es la propiedad que hace que la evidencia ausente, insuficiente o
/// contradictoria no pueda desembocar en una escritura automatica.
#[must_use]
pub fn evaluar(clasificacion: Clasificacion, perfil: PerfilSegmento) -> Veredicto {
    match clasificacion {
        Clasificacion::Clasificado {
            clase: Some(clase), ..
        } => Veredicto::Prohibida { clase },

        Clasificacion::Ambiguo { motivo } => Veredicto::RequiereAprobacion {
            motivo: Some(motivo),
        },

        // Sin evidencia de ninguna clase se escala igual que ante ambiguedad:
        // no saber nada de un dispositivo no es permiso para actuar sobre el.
        Clasificacion::NoClasificado => Veredicto::RequiereAprobacion {
            motivo: Some(MotivoAmbiguedad::SegmentoPuedeAlojarCriticos),
        },

        Clasificacion::Clasificado { clase: None, .. } => {
            if perfil.permite_respuesta_automatica() {
                Veredicto::Ejecutar
            } else {
                Veredicto::RequiereAprobacion { motivo: None }
            }
        }
    }
}

/// Orden de contencion emitida por el motor de respuesta.
#[derive(Debug, Clone)]
pub struct OrdenContencion {
    /// Identificador del nodo a contener.
    pub nodo: String,
    /// Mecanismo con el que se ejecutara.
    pub mecanismo: MecanismoContencion,
    /// Origen del evento que motivo la orden.
    pub origen: OrigenEvento,
    /// Justificacion registrada en ALM-01 junto con la accion.
    pub justificacion: String,
}

impl OrdenContencion {
    /// Valida la orden antes de su ejecucion.
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorGuardian::ContencionDeSimulacionRechazada`] si el evento
    /// procede de un simulacro.
    pub fn validar(&self) -> Result<(), ErrorGuardian> {
        if self.origen.habilita_contencion() {
            Ok(())
        } else {
            Err(ErrorGuardian::ContencionDeSimulacionRechazada)
        }
    }
}

#[cfg(test)]
mod pruebas {
    // Mismo criterio que crates/eje-ipc/src/pruebas.rs: en una prueba, abortar
    // con un mensaje util es la conducta correcta. La prohibicion de panic rige
    // en codigo de produccion, donde el guardian de inconclusos la vigila.
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::clasificacion::{
        Clasificacion, DeclaracionSegmento, Evidencia, FuenteEvidencia, MarcadoDispositivo,
        MotivoAmbiguedad, clasificar,
    };
    use super::*;

    fn orden(origen: OrigenEvento) -> OrdenContencion {
        OrdenContencion {
            nodo: "plc-linea-3".to_owned(),
            mecanismo: MecanismoContencion::SnmpV3,
            origen,
            justificacion: "prueba".to_owned(),
        }
    }

    #[test]
    fn contencion_de_simulacion_siempre_se_rechaza() {
        assert!(orden(OrigenEvento::Simulacion).validar().is_err());
    }

    #[test]
    fn contencion_de_produccion_se_admite() {
        assert!(orden(OrigenEvento::Produccion).validar().is_ok());
    }

    #[test]
    fn perfil_ot_nunca_emite_descubrimiento_activo() {
        assert!(!PerfilSegmento::Ot.permite_descubrimiento_activo());
        assert!(PerfilSegmento::Corporativo.permite_descubrimiento_activo());
    }

    // -----------------------------------------------------------------------
    // Paridad con contrato-contencion.toml — RPT-008
    // -----------------------------------------------------------------------

    /// Lee el manifiesto desde la raiz del workspace.
    fn manifiesto() -> String {
        let ruta = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("contrato-contencion.toml");

        std::fs::read_to_string(&ruta).unwrap_or_else(|error| {
            panic!(
                "no se pudo leer el manifiesto {}: {error}.\n\
                 contrato-contencion.toml describe la unica accion del producto que \
                 MODIFICA la red del cliente y debe estar versionado.",
                ruta.display()
            )
        })
    }

    /// Extrae los valores de `clave = "..."` que siguen a una cabecera de tabla.
    fn valores_bajo(contenido: &str, cabecera: &str, clave: &str) -> Vec<String> {
        let prefijo = format!("{clave} = \"");
        let mut valores = Vec::new();
        let mut dentro = false;

        for linea in contenido.lines() {
            let limpia = linea.trim();

            if limpia.starts_with('[') {
                dentro = limpia == cabecera;
                continue;
            }
            if limpia.starts_with('#') || !dentro {
                continue;
            }
            if let Some(resto) = limpia.strip_prefix(&prefijo) {
                if let Some(fin) = resto.find('"') {
                    valores.push(resto[..fin].to_owned());
                }
            }
        }

        valores
    }

    #[test]
    fn los_estados_coinciden_con_el_manifiesto() {
        let declarados = valores_bajo(&manifiesto(), "[[estado]]", "nombre");
        let implementados: Vec<String> = [
            EstadoContencion::Contenido,
            EstadoContencion::Rechazada,
            EstadoContencion::Desconocido,
        ]
        .iter()
        .map(|estado| estado.identificador().to_owned())
        .collect();

        assert_eq!(
            declarados, implementados,
            "los estados de contencion divergen del manifiesto.\n  \
             manifiesto: {declarados:?}\n  \
             codigo    : {implementados:?}"
        );
    }

    #[test]
    fn las_clases_excluidas_coinciden_con_el_manifiesto() {
        let declaradas = valores_bajo(&manifiesto(), "[[exclusion]]", "clase");
        let implementadas: Vec<String> = ClaseExcluida::TODAS
            .iter()
            .map(|clase| clase.identificador().to_owned())
            .collect();

        assert_eq!(
            declaradas, implementadas,
            "la lista de exclusion permanente diverge del manifiesto.\n  \
             manifiesto: {declaradas:?}\n  \
             codigo    : {implementadas:?}\n  \
             Una clase que solo exista en un lado deja de proteger sin que nadie lo vea."
        );
    }

    #[test]
    fn todo_mecanismo_declarado_existe_en_el_enum() {
        // Impide que un fabricante nuevo introduzca un transporte fuera del
        // vocabulario ratificado. Un borrador de este manifiesto declaraba
        // 'ssh-cli' y 'eapi', que no estan en MecanismoContencion.
        let declarados = valores_bajo(&manifiesto(), "[[fabricante]]", "mecanismo");

        assert!(
            !declarados.is_empty(),
            "el manifiesto debe declarar al menos un fabricante con su mecanismo"
        );

        let admitidos: Vec<&str> = MecanismoContencion::TODOS
            .iter()
            .map(|mecanismo| mecanismo.identificador())
            .collect();

        for nombre in declarados {
            assert!(
                admitidos.contains(&nombre.as_str()),
                "el mecanismo '{nombre}' no es una variante de MecanismoContencion. \
                 Admitidos: {admitidos:?}. La suplantacion ARP y SNMPv1/v2c estan \
                 prohibidos y por eso no figuran (RPT-003 §6.4)."
            );
        }
    }

    #[test]
    fn los_perfiles_del_manifiesto_coinciden_con_la_politica() {
        let contenido = manifiesto();
        let nombres = valores_bajo(&contenido, "[[perfil]]", "nombre");
        assert_eq!(nombres, vec!["corporativo", "ot"]);

        // El perfil OT debe declarar respuesta_automatica = false en el
        // manifiesto Y en el codigo. Que coincidan es el objeto de la prueba.
        let bloque_ot = contenido
            .split("[[perfil]]")
            .find(|bloque| bloque.contains("nombre = \"ot\""))
            .expect("el manifiesto debe declarar el perfil ot");

        assert!(
            bloque_ot.contains("respuesta_automatica = false"),
            "el manifiesto debe declarar que el perfil OT no admite respuesta automatica"
        );
        assert!(!PerfilSegmento::Ot.permite_respuesta_automatica());
        assert!(PerfilSegmento::Corporativo.permite_respuesta_automatica());
    }

    // -----------------------------------------------------------------------
    // Logica de decision — no depende de ningun conmutador
    // -----------------------------------------------------------------------

    #[test]
    fn la_exclusion_permanente_vence_al_perfil_corporativo() {
        // Es la prueba que justifica el orden de evaluacion: si el perfil se
        // comprobara primero, un corporativo ejecutaria sobre un dispositivo de
        // soporte vital.
        for clase in ClaseExcluida::TODAS {
            let clasificado = Clasificacion::Clasificado {
                clase: Some(clase),
                fuente: FuenteEvidencia::MarcadoAdministrativo,
            };
            assert_eq!(
                evaluar(clasificado, PerfilSegmento::Corporativo),
                Veredicto::Prohibida { clase }
            );
            assert_eq!(
                evaluar(clasificado, PerfilSegmento::Ot),
                Veredicto::Prohibida { clase }
            );
        }
    }

    /// Clasificacion contenible con respaldo humano.
    const fn contenible() -> Clasificacion {
        Clasificacion::Clasificado {
            clase: None,
            fuente: FuenteEvidencia::MarcadoAdministrativo,
        }
    }

    #[test]
    fn el_perfil_ot_nunca_ejecuta_sin_aprobacion() {
        assert_eq!(
            evaluar(contenible(), PerfilSegmento::Ot),
            Veredicto::RequiereAprobacion { motivo: None }
        );
    }

    #[test]
    fn el_perfil_corporativo_ejecuta_si_no_hay_exclusion() {
        assert_eq!(
            evaluar(contenible(), PerfilSegmento::Corporativo),
            Veredicto::Ejecutar
        );
    }

    #[test]
    fn el_estado_desconocido_escala_igual_que_el_rechazo() {
        // No saber si el puerto quedo aislado exige intervencion tanto como
        // saber que no lo quedo. Colapsar Desconocido en Contenido produce un
        // puerto que se cree aislado y no lo esta.
        assert!(EstadoContencion::Desconocido.escala_a_humano());
        assert!(EstadoContencion::Rechazada.escala_a_humano());
        assert!(!EstadoContencion::Contenido.escala_a_humano());
    }

    // -----------------------------------------------------------------------
    // Clasificacion — RPT-009, PA-23
    // -----------------------------------------------------------------------

    fn evidencia() -> Evidencia {
        Evidencia {
            marcado: None,
            segmento: DeclaracionSegmento::NoDeclarado,
            inferencia: None,
        }
    }

    #[test]
    fn las_fuentes_coinciden_con_el_manifiesto() {
        let declaradas = valores_bajo(&manifiesto(), "[[fuente_evidencia]]", "nombre");
        let implementadas: Vec<String> = FuenteEvidencia::TODAS
            .iter()
            .map(|fuente| fuente.identificador().to_owned())
            .collect();

        assert_eq!(
            declaradas, implementadas,
            "las fuentes de evidencia divergen del manifiesto.\n  \
             manifiesto: {declaradas:?}\n  \
             codigo    : {implementadas:?}"
        );
    }

    #[test]
    fn los_motivos_de_ambiguedad_coinciden_con_el_manifiesto() {
        let declarados = valores_bajo(&manifiesto(), "[[motivo_ambiguedad]]", "nombre");
        let implementados: Vec<String> = MotivoAmbiguedad::TODOS
            .iter()
            .map(|motivo| motivo.identificador().to_owned())
            .collect();

        assert_eq!(declarados, implementados);
    }

    #[test]
    fn solo_las_fuentes_declarativas_descartan_criticidad() {
        // La asimetria de la inferencia, comprobada contra el manifiesto: una
        // huella no puede demostrar que un equipo NO es critico.
        let contenido = manifiesto();

        for fuente in FuenteEvidencia::TODAS {
            let bloque = contenido
                .split("[[fuente_evidencia]]")
                .find(|bloque| bloque.contains(&format!("nombre = \"{}\"", fuente.identificador())))
                .expect("toda fuente debe figurar en el manifiesto");

            let declarado = bloque.contains("puede_declarar_no_critico = true");
            assert_eq!(
                declarado,
                fuente.puede_declarar_no_critico(),
                "'{}' declara puede_declarar_no_critico = {declarado} en el manifiesto \
                 y {} en el codigo",
                fuente.identificador(),
                fuente.puede_declarar_no_critico()
            );
        }
    }

    // --- Evidencia ausente ---

    #[test]
    fn sin_evidencia_alguna_no_se_contiene_automaticamente() {
        let clasificacion = clasificar(&evidencia());
        assert_eq!(
            clasificacion,
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::SegmentoPuedeAlojarCriticos
            }
        );
        assert!(!clasificacion.permite_accion_automatica());
    }

    #[test]
    fn un_segmento_no_declarado_se_trata_como_si_alojara_criticos() {
        // La ausencia de declaracion no es una declaracion de ausencia.
        assert!(DeclaracionSegmento::NoDeclarado.admite_criticos());
        assert!(DeclaracionSegmento::PuedeAlojarCriticos.admite_criticos());
        assert!(!DeclaracionSegmento::SinDispositivosCriticos.admite_criticos());
    }

    // --- Evidencia insuficiente ---

    #[test]
    fn la_inferencia_nunca_produce_prohibicion_permanente() {
        // Un falso positivo permanente e irrevocable seria tan malo como el
        // fallo que se quiere evitar: el dispositivo quedaria incontenible para
        // siempre sin que nadie pudiera corregirlo.
        let mut evidencia = evidencia();
        evidencia.inferencia = Some(ClaseExcluida::SoporteVital);

        let clasificacion = clasificar(&evidencia);
        assert_eq!(
            clasificacion,
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::InferenciaSugiereCriticidad
            }
        );

        // Y sobre todo: NO es Prohibida.
        assert!(!matches!(
            evaluar(clasificacion, PerfilSegmento::Corporativo),
            Veredicto::Prohibida { .. }
        ));
    }

    #[test]
    fn un_marcado_caducado_no_vale_como_marcado() {
        // Un marcado vencido se degrada a ausencia. Conservarlo como valido
        // seria afirmar algo sobre un parque que ya pudo cambiar.
        let mut evidencia = evidencia();
        evidencia.marcado = Some(MarcadoDispositivo {
            clase: None,
            vigente: false,
        });

        assert_eq!(
            clasificar(&evidencia),
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::MarcadoCaducado
            }
        );
    }

    // --- Evidencia contradictoria ---

    #[test]
    fn un_marcado_no_critico_contradicho_por_la_huella_es_ambiguo() {
        // El humano manda para PROHIBIR pero no para PERMITIR. Si el marcado
        // dice "no critico" y la huella dice lo contrario, o el marcado esta
        // obsoleto o el equipo fue sustituido. Ambas exigen mirar.
        let mut evidencia = evidencia();
        evidencia.marcado = Some(MarcadoDispositivo {
            clase: None,
            vigente: true,
        });
        evidencia.inferencia = Some(ClaseExcluida::SeguridadFuncional);

        assert_eq!(
            clasificar(&evidencia),
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::ConflictoEntreFuentes
            }
        );
    }

    #[test]
    fn un_marcado_critico_vence_a_la_inferencia_discrepante() {
        // La direccion contraria SI resuelve: anadir una prohibicion con una
        // firma humana es legitimo aunque la huella no la respalde.
        let mut evidencia = evidencia();
        evidencia.marcado = Some(MarcadoDispositivo {
            clase: Some(ClaseExcluida::SoporteVital),
            vigente: true,
        });
        evidencia.inferencia = None;

        assert_eq!(
            evaluar(clasificar(&evidencia), PerfilSegmento::Corporativo),
            Veredicto::Prohibida {
                clase: ClaseExcluida::SoporteVital
            }
        );
    }

    // --- El unico camino a la ejecucion automatica ---

    #[test]
    fn solo_un_segmento_declarado_limpio_permite_contener_sin_marcado() {
        let mut evidencia = evidencia();
        evidencia.segmento = DeclaracionSegmento::SinDispositivosCriticos;

        assert_eq!(
            evaluar(clasificar(&evidencia), PerfilSegmento::Corporativo),
            Veredicto::Ejecutar
        );
    }

    #[test]
    fn ninguna_evidencia_dudosa_desemboca_en_ejecucion() {
        // Barrido exhaustivo del espacio de entradas. Es la propiedad central de
        // PA-23 y merece comprobarse por enumeracion y no por argumento.
        let marcados = [
            None,
            Some(MarcadoDispositivo {
                clase: None,
                vigente: true,
            }),
            Some(MarcadoDispositivo {
                clase: None,
                vigente: false,
            }),
            Some(MarcadoDispositivo {
                clase: Some(ClaseExcluida::SoporteVital),
                vigente: true,
            }),
            Some(MarcadoDispositivo {
                clase: Some(ClaseExcluida::SoporteVital),
                vigente: false,
            }),
        ];
        let segmentos = [
            DeclaracionSegmento::SinDispositivosCriticos,
            DeclaracionSegmento::PuedeAlojarCriticos,
            DeclaracionSegmento::NoDeclarado,
        ];
        let inferencias = [None, Some(ClaseExcluida::SeguridadFuncional)];

        let mut ejecutadas = 0_u32;

        for marcado in marcados {
            for segmento in segmentos {
                for inferencia in inferencias {
                    let evidencia = Evidencia {
                        marcado,
                        segmento,
                        inferencia,
                    };
                    let clasificacion = clasificar(&evidencia);
                    let veredicto = evaluar(clasificacion, PerfilSegmento::Corporativo);

                    if veredicto == Veredicto::Ejecutar {
                        ejecutadas += 1;
                        // Toda ejecucion debe apoyarse en una clasificacion
                        // declarativa que descarte criticidad. Sin excepciones.
                        assert!(
                            clasificacion.permite_accion_automatica(),
                            "se ejecutaria sobre {evidencia:?} con clasificacion {clasificacion:?}"
                        );
                    }

                    // El perfil OT jamas ejecuta, sea cual sea la evidencia.
                    assert_ne!(
                        evaluar(clasificacion, PerfilSegmento::Ot),
                        Veredicto::Ejecutar,
                        "el perfil OT no debe ejecutar nunca; entrada: {evidencia:?}"
                    );
                }
            }
        }

        // Si esto bajara a cero, el producto no contendria nada y las pruebas de
        // arriba seguirian pasando: seria teatro en la direccion contraria.
        assert!(
            ejecutadas > 0,
            "ninguna combinacion permite contener; la politica es inaplicable"
        );
    }

    // -----------------------------------------------------------------------
    // Proveedores — RPT-010, PA-24
    // -----------------------------------------------------------------------

    use proveedores::{
        DireccionEnlace, ErrorProveedor, HistorialSegmento, Indicio, MarcadoVerificado,
        ProveedorHuella, ProveedorInventario, ProveedorOui, ProveedorSegmento, Proveedores,
        clasificar_con_proveedores,
    };

    const MAC: DireccionEnlace = [0x00, 0x1B, 0x21, 0x00, 0x00, 0x01];
    /// 5 de agosto de 2026, aproximado. Solo importan las diferencias.
    const AHORA: u64 = 1_785_888_000;

    /// Doble de inventario. Legitimo: es un banco de la propia logica, no una
    /// simulacion de un equipo de red de tercero (RPT-008 §2).
    struct InventarioDe(Result<Option<MarcadoVerificado>, ErrorProveedor>);
    impl ProveedorInventario for InventarioDe {
        fn marcado(
            &self,
            _mac: &DireccionEnlace,
        ) -> Result<Option<MarcadoVerificado>, ErrorProveedor> {
            self.0.clone()
        }
    }

    struct SegmentoDe(Result<HistorialSegmento, ErrorProveedor>);
    impl ProveedorSegmento for SegmentoDe {
        fn historial(&self, _mac: &DireccionEnlace) -> Result<HistorialSegmento, ErrorProveedor> {
            self.0.clone()
        }
    }

    struct OuiDe(Result<Indicio, ErrorProveedor>);
    impl ProveedorOui for OuiDe {
        fn indicio(&self, _mac: &DireccionEnlace) -> Result<Indicio, ErrorProveedor> {
            self.0.clone()
        }
    }

    struct HuellaDe(Result<Indicio, ErrorProveedor>);
    impl ProveedorHuella for HuellaDe {
        fn indicio(&self, _mac: &DireccionEnlace) -> Result<Indicio, ErrorProveedor> {
            self.0.clone()
        }
    }

    fn segmento_limpio() -> HistorialSegmento {
        HistorialSegmento {
            actual: DeclaracionSegmento::SinDispositivosCriticos,
            visto_en_segmento_critico: false,
        }
    }

    // --- PA24-UT-01: firma alterada o inclusion no probada ---

    #[test]
    fn una_firma_invalida_no_se_lee_como_marcado_ausente() {
        // Es la distincion central: «no hay marcado» permite contener en un
        // segmento limpio; «el marcado no verifica» nunca lo permite.
        for error in [
            ErrorProveedor::FirmaInvalida {
                detalle: "prueba".to_owned(),
            },
            ErrorProveedor::InclusionNoProbada,
        ] {
            let inventario = InventarioDe(Err(error.clone()));
            let proveedores = Proveedores {
                inventario: &inventario,
                segmento: &SegmentoDe(Ok(segmento_limpio())),
                oui: &OuiDe(Ok(Indicio::SinIndicio)),
                huella: &HuellaDe(Ok(Indicio::SinIndicio)),
            };

            assert_eq!(
                clasificar_con_proveedores(&proveedores, &MAC, AHORA),
                Clasificacion::Ambiguo {
                    motivo: MotivoAmbiguedad::EvidenciaNoVerificable
                },
                "el error {error:?} no debe confundirse con ausencia de marcado"
            );
        }
    }

    #[test]
    fn el_ataque_de_supresion_no_produce_permiso() {
        // Suprimir la entrada «esto es soporte vital» del inventario deja las
        // firmas restantes intactas. Lo que no deja intacta es la prueba de
        // inclusion contra la raiz anclada, y ese es el motivo de que la
        // verificacion no pueda ser solo por entrada.
        let inventario = InventarioDe(Err(ErrorProveedor::InclusionNoProbada));
        let proveedores = Proveedores {
            inventario: &inventario,
            segmento: &SegmentoDe(Ok(segmento_limpio())),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SinIndicio)),
        };

        let veredicto = evaluar(
            clasificar_con_proveedores(&proveedores, &MAC, AHORA),
            PerfilSegmento::Corporativo,
        );
        assert_ne!(veredicto, Veredicto::Ejecutar);
        assert!(veredicto.exige_alerta());
    }

    // --- PA24-UT-02: OUI generico con protocolo industrial ---

    #[test]
    fn la_huella_eleva_la_criticidad_pese_a_un_oui_comercial() {
        let proveedores = Proveedores {
            inventario: &InventarioDe(Ok(None)),
            segmento: &SegmentoDe(Ok(segmento_limpio())),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SugiereCriticidad(
                ClaseExcluida::SeguridadFuncional,
            ))),
        };

        assert_eq!(
            clasificar_con_proveedores(&proveedores, &MAC, AHORA),
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::InferenciaSugiereCriticidad
            }
        );
    }

    // --- PA24-UT-04: equipo rodante ---

    #[test]
    fn la_ambiguedad_de_segmento_es_pegajosa() {
        // Un carro de telemedicina que paso por la VLAN clinica y aparece ahora
        // en la administrativa no debe volverse contenible por haberse movido.
        let historial = HistorialSegmento {
            actual: DeclaracionSegmento::SinDispositivosCriticos,
            visto_en_segmento_critico: true,
        };
        assert_eq!(
            historial.declaracion_efectiva(),
            DeclaracionSegmento::PuedeAlojarCriticos
        );

        let proveedores = Proveedores {
            inventario: &InventarioDe(Ok(None)),
            segmento: &SegmentoDe(Ok(historial)),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SinIndicio)),
        };

        assert_eq!(
            clasificar_con_proveedores(&proveedores, &MAC, AHORA),
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::SegmentoPuedeAlojarCriticos
            }
        );
    }

    // --- Asimetria de los fallos de proveedor ---

    #[test]
    fn un_fallo_declarativo_bloquea() {
        let proveedores = Proveedores {
            inventario: &InventarioDe(Ok(None)),
            segmento: &SegmentoDe(Err(ErrorProveedor::FuenteInaccesible {
                fuente: "registro-de-segmentos".to_owned(),
            })),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SinIndicio)),
        };

        assert_eq!(
            clasificar_con_proveedores(&proveedores, &MAC, AHORA),
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::EvidenciaNoVerificable
            }
        );
    }

    #[test]
    fn un_fallo_inferido_no_bloquea() {
        // Si tumbar la captura inutilizara la contencion en toda la red, el
        // producto seria fragil ante un atacante que solo tuviera que apagar una
        // fuente. El permiso vino de una fuente declarativa; la inferencia nunca
        // lo concedio, asi que su ausencia no puede retirarlo (RPT-009 §3).
        let proveedores = Proveedores {
            inventario: &InventarioDe(Ok(None)),
            segmento: &SegmentoDe(Ok(segmento_limpio())),
            oui: &OuiDe(Err(ErrorProveedor::FuenteInaccesible {
                fuente: "oui".to_owned(),
            })),
            huella: &HuellaDe(Err(ErrorProveedor::FuenteInaccesible {
                fuente: "captura".to_owned(),
            })),
        };

        assert_eq!(
            evaluar(
                clasificar_con_proveedores(&proveedores, &MAC, AHORA),
                PerfilSegmento::Corporativo
            ),
            Veredicto::Ejecutar
        );
    }

    // -----------------------------------------------------------------------
    // Inventario firmado — RPT-011
    // -----------------------------------------------------------------------

    use eje_almacen::merkle::{PruebaInclusion, prueba_inclusion};
    use eje_almacen::resumen::Resumen;
    use inventario::{
        Centinela, ClaveInventario, DominioClave, ErrorInventario, Inventario, MarcadoBruto,
        RaizAnclada, RaizVerificada, mensaje_de_raiz,
    };
    use motor_pqc::firma_hibrida::{
        ClaveFirmaHibrida, ClaveVerificacionHibrida, FirmaHibrida, firmar, generar_par,
    };
    use vlan::{
        DeclaracionVlan, ErrorVlan, NaturalezaSegmento, TablaVlan, TablaVlanVerificada,
        VLAN_MAXIMA, VLAN_MINIMA,
    };

    /// Generador determinista para pruebas reproducibles.
    ///
    /// Replica el de `motor-pqc`. No se exporta desde alli a proposito: un
    /// generador de pruebas en la API publica de un crate criptografico es una
    /// invitacion a usarlo fuera de las pruebas.
    struct GeneradorDeterminista {
        estado: u64,
    }

    impl GeneradorDeterminista {
        const fn nuevo(semilla: u64) -> Self {
            Self { estado: semilla }
        }

        /// xorshift64*, suficiente para reproducibilidad en pruebas.
        fn siguiente(&mut self) -> u64 {
            self.estado ^= self.estado >> 12;
            self.estado ^= self.estado << 25;
            self.estado ^= self.estado >> 27;
            self.estado.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
    }

    impl rand_core::TryRng for GeneradorDeterminista {
        type Error = rand_core::Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok((self.siguiente() >> 32) as u32)
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(self.siguiente())
        }

        fn try_fill_bytes(&mut self, destino: &mut [u8]) -> Result<(), Self::Error> {
            for trozo in destino.chunks_mut(8) {
                let valor = self.siguiente().to_le_bytes();
                let longitud = trozo.len();
                trozo.copy_from_slice(&valor[..longitud]);
            }
            Ok(())
        }
    }

    impl rand_core::TryCryptoRng for GeneradorDeterminista {}

    /// Inventario de prueba con su clave, raiz y firma.
    ///
    /// Las pruebas recorren la cadena real: generan clave, construyen el arbol,
    /// firman la raiz y piden la prueba de inclusion. No existe atajo para
    /// fabricar un `MarcadoVerificado`, que es justamente el punto.
    struct Banco {
        inventario: Inventario,
        vlans: TablaVlan,
        anclada: RaizAnclada,
        firma: FirmaHibrida,
        clave: ClaveInventario,
        centinela: Centinela,
        revocaciones: RegistroRevocaciones,
    }

    /// VLAN clinica del banco: declarada como capaz de alojar criticos.
    const VLAN_CLINICA: u16 = 40;
    /// VLAN administrativa del banco: declarada limpia.
    const VLAN_LIMPIA: u16 = 10;

    /// Tabla de segmentos de prueba. Se pasa **desordenada** a proposito.
    fn segmentos_de_prueba() -> Vec<DeclaracionVlan> {
        vec![
            DeclaracionVlan {
                vlan: VLAN_CLINICA,
                naturaleza: NaturalezaSegmento::PuedeAlojarCriticos,
                emitido_en: AHORA,
                vigencia_dias: 365,
            },
            DeclaracionVlan {
                vlan: VLAN_LIMPIA,
                naturaleza: NaturalezaSegmento::SinDispositivosCriticos,
                emitido_en: AHORA,
                vigencia_dias: 365,
            },
        ]
    }

    /// Marcados de prueba. Se pasan **desordenados** a proposito: el orden
    /// canonico lo impone `Inventario::construir`, no quien los escribe.
    fn marcados_de_prueba() -> Vec<MarcadoBruto> {
        vec![
            MarcadoBruto {
                mac: [0x00, 0x1B, 0x21, 0x00, 0x00, 0x03],
                clase: Some(ClaseExcluida::SeguridadFuncional),
                emitido_en: AHORA,
                vigencia_dias: 365,
            },
            MarcadoBruto {
                mac: MAC,
                clase: Some(ClaseExcluida::SoporteVital),
                emitido_en: AHORA,
                vigencia_dias: 365,
            },
            MarcadoBruto {
                mac: [0x00, 0x1B, 0x21, 0x00, 0x00, 0x02],
                clase: None,
                emitido_en: AHORA,
                vigencia_dias: 365,
            },
        ]
    }

    /// Semilla del par operativo por defecto.
    const SEMILLA_OPERATIVA: u64 = 0x45_4A_45;
    /// Semilla del par sucesor, para las rotaciones.
    const SEMILLA_SUCESORA: u64 = 0x53_55_43;

    fn firmar_raiz_con(
        anclada: &RaizAnclada,
        semilla: u64,
    ) -> (FirmaHibrida, ClaveVerificacionHibrida) {
        let (firmante, verificadora) = generar_par(&mut GeneradorDeterminista::nuevo(semilla));
        let firma = motor_pqc::firma_hibrida::firmar(&firmante, &mensaje_de_raiz(anclada));
        (firma, verificadora)
    }

    fn banco_con_semilla(
        dominio: DominioClave,
        secuencia: u64,
        centinela: Centinela,
        semilla: u64,
    ) -> Banco {
        let inventario =
            Inventario::construir(marcados_de_prueba()).expect("no hay direcciones repetidas");
        let vlans = TablaVlan::construir(segmentos_de_prueba()).expect("no hay vlans repetidas");
        let anclada = RaizAnclada {
            raiz: inventario.raiz().expect("el inventario no esta vacio"),
            vlans: vlans.resumen(),
            secuencia,
        };
        let (firma, verificadora) = firmar_raiz_con(&anclada, semilla);

        Banco {
            inventario,
            vlans,
            anclada,
            firma,
            clave: ClaveInventario::nueva(verificadora, dominio),
            centinela,
            revocaciones: RegistroRevocaciones::nuevo(),
        }
    }

    fn banco_con(dominio: DominioClave, secuencia: u64, centinela: Centinela) -> Banco {
        banco_con_semilla(dominio, secuencia, centinela, SEMILLA_OPERATIVA)
    }

    fn banco(dominio: DominioClave) -> Banco {
        banco_con(dominio, 7, Centinela::Establecido(7))
    }

    impl Banco {
        fn raiz_verificada(&self) -> Result<RaizVerificada, ErrorInventario> {
            RaizVerificada::verificar(
                self.anclada,
                &self.firma,
                &self.clave,
                self.centinela,
                &self.revocaciones,
            )
        }

        /// Posicion canonica de una direccion en el inventario ordenado.
        fn posicion(&self, mac: &[u8; 6]) -> usize {
            self.inventario.posicion_de(mac).expect("la mac figura")
        }

        fn marcado(&self, posicion: usize) -> MarcadoBruto {
            self.inventario.marcados()[posicion]
        }

        fn prueba_de(&self, posicion: usize) -> PruebaInclusion {
            prueba_inclusion(&self.inventario.resumenes(), posicion, posicion as u64)
                .expect("la posicion existe")
        }

        fn verificar(&self, posicion: usize) -> Result<MarcadoVerificado, ErrorInventario> {
            let raiz = self.raiz_verificada()?;
            MarcadoVerificado::verificar_e_instanciar(
                self.marcado(posicion),
                &self.prueba_de(posicion),
                &raiz,
            )
        }
    }

    #[test]
    fn un_marcado_integro_verifica() {
        let banco = banco(DominioClave::Cliente);
        let posicion = banco.posicion(&MAC);
        let marcado = banco.verificar(posicion).expect("la cadena esta completa");

        assert_eq!(marcado.clase(), Some(ClaseExcluida::SoporteVital));
        assert_eq!(marcado.mac(), &MAC);
    }

    #[test]
    fn una_prueba_de_otra_entrada_se_rechaza() {
        // El eslabon que se olvida. `verificar_inclusion` comprueba que la
        // prueba es consistente con la raiz, pero nada en ella la ata al marcado
        // que se esta verificando: sin este control, quien presente la prueba
        // legitima de otra entrada podria colarla como si fuera esta.
        let banco = banco(DominioClave::Cliente);
        let raiz = banco.raiz_verificada().expect("la raiz verifica");
        let posicion = banco.posicion(&MAC);

        let resultado = MarcadoVerificado::verificar_e_instanciar(
            banco.marcado(posicion),
            &banco.prueba_de(posicion + 1),
            &raiz,
        );

        assert_eq!(resultado, Err(ErrorInventario::PruebaAjenaAlMarcado));
    }

    #[test]
    fn alterar_el_marcado_rompe_la_cadena() {
        let banco = banco(DominioClave::Cliente);
        let raiz = banco.raiz_verificada().expect("la raiz verifica");
        let posicion = banco.posicion(&MAC);

        // Degradar «soporte vital» a «no critico» es el ataque util.
        let mut alterado = banco.marcado(posicion);
        alterado.clase = None;

        let resultado =
            MarcadoVerificado::verificar_e_instanciar(alterado, &banco.prueba_de(posicion), &raiz);

        assert_eq!(resultado, Err(ErrorInventario::PruebaAjenaAlMarcado));
    }

    #[test]
    fn suprimir_una_entrada_invalida_la_firma_de_la_raiz() {
        // RPT-010 §4 comprobado de extremo a extremo: con firma por entrada,
        // borrar «esta bomba es soporte vital» no rompia nada. Con la raiz
        // firmada, la raiz del inventario mutilado ya no es la firmada.
        let banco = banco(DominioClave::Cliente);
        let posicion = banco.posicion(&MAC);

        let supervivientes: Vec<MarcadoBruto> = banco
            .inventario
            .marcados()
            .iter()
            .enumerate()
            .filter(|(indice, _)| *indice != posicion)
            .map(|(_, marcado)| *marcado)
            .collect();
        let mutilado = Inventario::construir(supervivientes).expect("sigue sin duplicados");
        let raiz_mutilada = mutilado.raiz().expect("quedan entradas");

        assert_ne!(raiz_mutilada, banco.anclada.raiz);

        let resultado = RaizVerificada::verificar(
            RaizAnclada {
                raiz: raiz_mutilada,
                vlans: banco.anclada.vlans,
                secuencia: banco.anclada.secuencia,
            },
            &banco.firma,
            &banco.clave,
            banco.centinela,
            &banco.revocaciones,
        );

        assert_eq!(resultado, Err(ErrorInventario::FirmaDeRaizInvalida));
    }

    #[test]
    fn una_raiz_ajena_no_verifica() {
        // La raiz firmada es legitima, pero la prueba pertenece a otro arbol.
        let banco = banco(DominioClave::Cliente);
        let otra = Inventario::construir(vec![MarcadoBruto {
            mac: [0xAA; 6],
            clase: None,
            emitido_en: AHORA,
            vigencia_dias: 1,
        }])
        .expect("una sola entrada");

        let raiz = banco.raiz_verificada().expect("la raiz verifica");
        let prueba = prueba_inclusion(&otra.resumenes(), 0, 0).expect("la posicion existe");

        let resultado =
            MarcadoVerificado::verificar_e_instanciar(otra.marcados()[0], &prueba, &raiz);

        assert_eq!(resultado, Err(ErrorInventario::InclusionNoVerifica));
    }

    // --- PA-27: reversion y frescura ---

    #[test]
    fn un_inventario_anterior_se_rechaza_pese_a_firma_valida() {
        // El ataque de PA-27. Quien compromete el almacen no falsifica nada:
        // restaura el fichero legitimo de la semana pasada, emitido antes de que
        // la bomba se marcara como soporte vital.
        let banco = banco_con(DominioClave::Cliente, 6, Centinela::Establecido(9));

        assert_eq!(
            banco.raiz_verificada(),
            Err(ErrorInventario::ReversionDetectada {
                aceptada: 9,
                presentada: 6,
            })
        );
    }

    #[test]
    fn la_misma_secuencia_se_admite_y_una_posterior_tambien() {
        // Reemitir sin cambios no es un ataque; retroceder si.
        assert!(
            banco_con(DominioClave::Cliente, 9, Centinela::Establecido(9))
                .raiz_verificada()
                .is_ok()
        );
        assert!(
            banco_con(DominioClave::Cliente, 10, Centinela::Establecido(9))
                .raiz_verificada()
                .is_ok()
        );
    }

    #[test]
    fn borrar_el_centinela_no_se_lee_como_primera_vez() {
        // Si la ausencia de centinela significara «primera vez», bastaria
        // borrarlo para desactivar toda la proteccion de frescura. Borrarlo debe
        // ser tan detectable como rebobinarlo.
        let banco = banco_con(DominioClave::Cliente, 1, Centinela::SinEstablecer);

        assert_eq!(
            banco.raiz_verificada(),
            Err(ErrorInventario::FrescuraNoEstablecida)
        );
    }

    #[test]
    fn el_aprovisionamiento_inicial_establece_el_centinela() {
        let banco = banco_con(DominioClave::Cliente, 4, Centinela::SinEstablecer);
        let (raiz, centinela) =
            RaizVerificada::aprovisionar(banco.anclada, &banco.firma, &banco.clave)
                .expect("el aprovisionamiento ocurre con un humano presente");

        assert_eq!(raiz.secuencia(), 4);
        assert_eq!(centinela, Centinela::Establecido(4));
    }

    #[test]
    fn el_centinela_nunca_retrocede() {
        assert_eq!(
            Centinela::Establecido(9).avanzar(3),
            Centinela::Establecido(9)
        );
        assert_eq!(
            Centinela::Establecido(9).avanzar(11),
            Centinela::Establecido(11)
        );
        assert_eq!(
            Centinela::SinEstablecer.avanzar(2),
            Centinela::Establecido(2)
        );
    }

    #[test]
    fn la_secuencia_viaja_dentro_del_mensaje_firmado() {
        // Firmar la raiz por un lado y la secuencia por otro permitiria
        // recombinar la raiz vieja con la secuencia nueva.
        let raiz = Resumen::desde_bytes([9u8; 32]);
        let vlans = TablaVlan::vacia().resumen();
        let uno = mensaje_de_raiz(&RaizAnclada {
            raiz,
            vlans,
            secuencia: 1,
        });
        let dos = mensaje_de_raiz(&RaizAnclada {
            raiz,
            vlans,
            secuencia: 2,
        });

        assert_ne!(uno, dos);
    }

    #[test]
    fn recombinar_raiz_vieja_con_secuencia_nueva_no_verifica() {
        let vieja = banco_con(DominioClave::Cliente, 5, Centinela::Establecido(5));

        // Se conserva la firma de la secuencia 5 y se declara la 12.
        let resultado = RaizVerificada::verificar(
            RaizAnclada {
                raiz: vieja.anclada.raiz,
                vlans: vieja.anclada.vlans,
                secuencia: 12,
            },
            &vieja.firma,
            &vieja.clave,
            Centinela::Establecido(5),
            &RegistroRevocaciones::nuevo(),
        );

        assert_eq!(resultado, Err(ErrorInventario::FirmaDeRaizInvalida));
    }

    // --- Orden canonico y duplicados ---

    #[test]
    fn el_orden_de_entrada_no_altera_la_raiz() {
        // Dos herramientas administrativas que enumeren los equipos en orden
        // distinto deben producir la misma raiz.
        let uno = Inventario::construir(marcados_de_prueba()).expect("sin duplicados");
        let mut invertidos = marcados_de_prueba();
        invertidos.reverse();
        let otro = Inventario::construir(invertidos).expect("sin duplicados");

        assert_eq!(uno.raiz(), otro.raiz());
        assert_eq!(uno.marcados(), otro.marcados());
    }

    // -----------------------------------------------------------------------
    // Formato en disco y extremo a extremo — RPT-013
    // -----------------------------------------------------------------------

    use almacen::{ErrorCarga, InventarioLocal};
    use formato::{ENTRADAS_MAXIMAS, ErrorFormato, MAGICO, analizar, serializar};

    /// Longitud de la cabecera del fichero: magico, version, secuencia,
    /// entradas y segmentos.
    ///
    /// Las pruebas que parchean bytes concretos la necesitan. Se declara una vez
    /// aqui porque estaba escrita a mano en tres sitios, y al ampliar la
    /// cabecera en la version 2 esos tres numeros habrian quedado apuntando al
    /// campo equivocado con las pruebas todavia en verde.
    const CABECERA: usize = 24;

    /// Longitud de una entrada de marcado.
    const ENTRADA: usize = 19;

    /// Longitud de un registro de declaracion de segmento.
    const SEGMENTO: usize = 15;

    /// Bytes tal como quedarian en disco, para el banco dado.
    fn bytes_en_disco(banco: &Banco) -> Vec<u8> {
        serializar(
            &banco.inventario,
            &banco.vlans,
            banco.anclada.secuencia,
            &banco.firma,
        )
    }

    #[test]
    fn el_recorrido_completo_de_fichero_a_veredicto() {
        // La prueba que faltaba: cinco reportes de diseno ejercitados de extremo
        // a extremo sobre un artefacto real.
        let banco = banco(DominioClave::Cliente);
        let bytes = bytes_en_disco(&banco);

        let local =
            InventarioLocal::cargar(&bytes, &banco.clave, banco.centinela, &banco.revocaciones)
                .expect("el fichero recien escrito debe cargar");

        assert_eq!(local.entradas(), 3);
        assert_eq!(local.secuencia(), banco.anclada.secuencia);

        // De la MAC al veredicto, pasando por el proveedor.
        let proveedores = Proveedores {
            inventario: &local,
            segmento: &SegmentoDe(Ok(segmento_limpio())),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SinIndicio)),
        };

        let veredicto = evaluar(
            clasificar_con_proveedores(&proveedores, &MAC, AHORA),
            PerfilSegmento::Corporativo,
        );

        assert_eq!(
            veredicto,
            Veredicto::Prohibida {
                clase: ClaseExcluida::SoporteVital
            }
        );
        assert!(veredicto.es_amenaza_incontenible());
    }

    #[test]
    fn un_dispositivo_ausente_del_fichero_no_es_un_fallo() {
        // Ausencia legitima frente a fallo de verificacion: RPT-010 §4 obliga a
        // no confundirlos, y aqui es donde se decide.
        let banco = banco(DominioClave::Cliente);
        let local = InventarioLocal::cargar(
            &bytes_en_disco(&banco),
            &banco.clave,
            banco.centinela,
            &banco.revocaciones,
        )
        .expect("carga");

        assert_eq!(local.marcado(&[0xFF; 6]), Ok(None));
    }

    #[test]
    fn el_fichero_es_reversible() {
        let banco = banco(DominioClave::Cliente);
        let fichero = analizar(&bytes_en_disco(&banco)).expect("analiza");

        assert_eq!(fichero.inventario, banco.inventario);
        assert_eq!(fichero.anclada, banco.anclada);
    }

    #[test]
    fn la_raiz_no_viaja_en_el_fichero_sino_que_se_recalcula() {
        // Guardarla crearia una pregunta que no debe existir: si la raiz escrita
        // y la recalculada discrepan, cual vale. Cualquier respuesta es
        // explotable, asi que no se escribe.
        let banco = banco(DominioClave::Cliente);
        let bytes = bytes_en_disco(&banco);

        assert!(
            !bytes
                .windows(32)
                .any(|ventana| ventana == banco.anclada.raiz.bytes()),
            "la raiz no debe aparecer literalmente en el fichero"
        );

        // Mismo argumento para el resumen de segmentos: si viajara escrito, un
        // fichero con la tabla alterada y el resumen viejo plantearia la misma
        // pregunta sin respuesta segura.
        assert!(
            !bytes
                .windows(32)
                .any(|ventana| ventana == banco.anclada.vlans.bytes()),
            "el resumen de segmentos tampoco debe aparecer literalmente"
        );

        let fichero = analizar(&bytes).expect("analiza");
        assert_eq!(
            formato::raiz_recalculada(&fichero),
            Some(banco.anclada.raiz)
        );
        assert_eq!(fichero.anclada.vlans, banco.vlans.resumen());
    }

    // --- Analizador defensivo: el frente no autenticado ---

    #[test]
    fn un_fichero_vacio_o_minusculo_no_desborda() {
        // 24 es la cabecera de la version 2: magico, version, secuencia,
        // entradas y segmentos.
        for longitud in 0..24 {
            let resultado = analizar(&vec![0u8; longitud]).err();
            assert!(
                matches!(resultado, Some(ErrorFormato::Truncado { .. })),
                "longitud {longitud} deberia dar truncado, dio {resultado:?}"
            );
        }
    }

    #[test]
    fn un_magico_ajeno_se_rechaza() {
        let mut bytes = bytes_en_disco(&banco(DominioClave::Cliente));
        bytes[0] = b'X';

        assert_eq!(analizar(&bytes).err(), Some(ErrorFormato::MagicoAusente));
    }

    #[test]
    fn una_version_futura_se_rechaza_en_lugar_de_interpretarse() {
        // Interpretar un formato que no se conoce es adivinar sobre entrada
        // hostil.
        let mut bytes = bytes_en_disco(&banco(DominioClave::Cliente));
        bytes[8..10].copy_from_slice(&9u16.to_be_bytes());

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::VersionDesconocida { encontrada: 9 })
        );
    }

    #[test]
    fn un_numero_de_entradas_absurdo_no_reserva_memoria() {
        // El ataque clasico: una cabecera que declara cuatro mil millones de
        // entradas. Se acota ANTES de multiplicar o reservar.
        //
        // La version se toma de `formato::VERSION` y no se escribe a mano: con
        // un literal, subir la version convertiria esta prueba en una
        // comprobacion de obsolescencia sin que nadie lo notara.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGICO);
        bytes.extend_from_slice(&formato::VERSION.to_be_bytes());
        bytes.extend_from_slice(&1u64.to_be_bytes());
        bytes.extend_from_slice(&u32::MAX.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::DemasiadasEntradas {
                declaradas: u32::MAX as usize
            })
        );
        assert!(ENTRADAS_MAXIMAS < u32::MAX as usize);
    }

    #[test]
    fn un_numero_de_segmentos_absurdo_no_reserva_memoria() {
        // El mismo ataque por el campo nuevo. Que el contador sea `u16` acota el
        // dano por si solo, pero apoyarse en el ancho del entero deja la
        // proteccion a merced de un cambio de tipo futuro.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGICO);
        bytes.extend_from_slice(&formato::VERSION.to_be_bytes());
        bytes.extend_from_slice(&1u64.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&u16::MAX.to_be_bytes());

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::DemasiadosSegmentos {
                declaradas: u16::MAX as usize
            })
        );
        assert!(vlan::VLANS_MAXIMAS < u16::MAX as usize);
    }

    #[test]
    fn un_fichero_truncado_se_detecta() {
        let bytes = bytes_en_disco(&banco(DominioClave::Cliente));

        for recorte in [1, 64, 1000] {
            let cortado = &bytes[..bytes.len() - recorte];
            assert!(
                matches!(analizar(cortado).err(), Some(ErrorFormato::Truncado { .. })),
                "recortar {recorte} bytes deberia dar truncado"
            );
        }
    }

    #[test]
    fn los_bytes_sobrantes_se_rechazan() {
        // Un fichero cuya cola no se interpreta admite dos lecturas: la del
        // analizador y la de quien anadio los bytes.
        let mut bytes = bytes_en_disco(&banco(DominioClave::Cliente));
        bytes.extend_from_slice(b"cola");

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::BytesSobrantes { sobrantes: 4 })
        );
    }

    #[test]
    fn un_codigo_de_clase_desconocido_se_rechaza() {
        // Aceptarlo como «no critico» seria dar al atacante una via de
        // degradacion mediante un byte que el analizador no entiende.
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);
        bytes[CABECERA + 6] = 200;

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::ClaseDesconocida { codigo: 200 })
        );
    }

    #[test]
    fn alterar_una_entrada_del_fichero_invalida_la_firma() {
        // El fichero sigue bien formado; lo que falla es la criptografia. Las dos
        // capas se distinguen en el tipo de error.
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);

        // Degradar la primera entrada a «no critico».
        let posicion_clase = CABECERA + banco.posicion(&MAC) * ENTRADA + 6;
        bytes[posicion_clase] = 0;

        assert!(analizar(&bytes).is_ok(), "el fichero sigue bien formado");
        assert_eq!(
            InventarioLocal::cargar(&bytes, &banco.clave, banco.centinela, &banco.revocaciones)
                .err(),
            Some(ErrorCarga::Verificacion(
                ErrorInventario::FirmaDeRaizInvalida
            ))
        );
    }

    #[test]
    fn un_fichero_revertido_no_carga() {
        // PA-27 comprobado desde disco.
        let banco = banco_con(DominioClave::Cliente, 6, Centinela::Establecido(9));

        assert_eq!(
            InventarioLocal::cargar(
                &bytes_en_disco(&banco),
                &banco.clave,
                banco.centinela,
                &banco.revocaciones
            )
            .err(),
            Some(ErrorCarga::Verificacion(
                ErrorInventario::ReversionDetectada {
                    aceptada: 9,
                    presentada: 6
                }
            ))
        );
    }

    #[test]
    fn el_orden_en_disco_no_altera_el_resultado() {
        // Dos ficheros con las entradas escritas en orden distinto deben
        // producir el mismo inventario y la misma raiz.
        let banco = banco(DominioClave::Cliente);
        let uno = bytes_en_disco(&banco);

        let mut invertido = banco.inventario.marcados().to_vec();
        invertido.reverse();
        let mut segmentos_invertidos = banco.vlans.declaraciones().to_vec();
        segmentos_invertidos.reverse();

        let otro = serializar(
            &Inventario::construir(invertido).expect("sin duplicados"),
            &TablaVlan::construir(segmentos_invertidos).expect("sin vlans repetidas"),
            banco.anclada.secuencia,
            &banco.firma,
        );

        assert_eq!(uno, otro, "construir reordena, asi que los bytes coinciden");
    }

    // -----------------------------------------------------------------------
    // Almacen de observacion — RPT-018 §6, PA-38
    // -----------------------------------------------------------------------

    use observacion::{AlmacenObservacion, CAPACIDAD_PEGAJOSA, CAPACIDAD_VOLATIL, Protocolo};

    /// Direccion derivada de un indice, para llenar tablas.
    fn mac_numero(numero: u32) -> DireccionEnlace {
        let bytes = numero.to_be_bytes();
        [0x02, 0x00, bytes[0], bytes[1], bytes[2], bytes[3]]
    }

    // -----------------------------------------------------------------------
    // Enumeracion del inventario — RPT-087, PA-138a
    // -----------------------------------------------------------------------

    /// El almacen sabia contar y no sabia enumerar.
    ///
    /// Ese era el bloqueo entero de PA-138: `volatiles()` y `pegajosos()`
    /// devuelven `usize`, y un canal que promete una lista de dispositivos no se
    /// puede escribir contra un almacen que solo dice cuantos hay.
    #[test]
    fn el_inventario_enumera_lo_observado_y_no_solo_lo_cuenta() {
        let mut almacen = AlmacenObservacion::nuevo();

        almacen.observar(
            MAC,
            Some(Protocolo::Hl7),
            DeclaracionSegmento::PuedeAlojarCriticos,
        );
        almacen.observar(
            mac_numero(7),
            Some(Protocolo::Modbus),
            DeclaracionSegmento::SinDispositivosCriticos,
        );

        let inventario = almacen.inventario();

        assert_eq!(inventario.len(), almacen.volatiles());

        let clinico = inventario
            .iter()
            .find(|nodo| nodo.direccion == MAC)
            .expect("el equipo clinico esta en la lista");

        assert_eq!(clinico.protocolos, vec![Protocolo::Hl7]);
        assert_eq!(clinico.segmento, DeclaracionSegmento::PuedeAlojarCriticos);
        assert!(
            clinico.pegajoso,
            "se le vio en un segmento que admite criticos"
        );
        assert!(clinico.visto_en > 0, "el reloj de la observacion viaja");
    }

    /// Dos consultas seguidas devuelven el mismo orden.
    ///
    /// `HashMap` no promete ninguno. Un inventario que se reordena solo hace
    /// parpadear la pantalla del operador y arruina comparar una vuelta con la
    /// siguiente.
    #[test]
    fn el_orden_del_inventario_es_estable_entre_consultas() {
        let mut almacen = AlmacenObservacion::nuevo();

        for numero in 0..32 {
            almacen.observar(
                mac_numero(numero),
                None,
                DeclaracionSegmento::SinDispositivosCriticos,
            );
        }

        let primera = almacen.inventario();
        let segunda = almacen.inventario();

        assert_eq!(primera, segunda);

        let mut ordenadas: Vec<DireccionEnlace> =
            primera.iter().map(|nodo| nodo.direccion).collect();
        ordenadas.sort_unstable();

        assert_eq!(
            primera
                .iter()
                .map(|nodo| nodo.direccion)
                .collect::<Vec<_>>(),
            ordenadas,
            "el orden sale de la clave, que es lo unico estable"
        );
    }

    /// El tercer estado del inventario, y el que costo abrir el punto.
    ///
    /// Un pegajoso expulsado del volatil **no** se afirma presente: de el se sabe
    /// que estuvo en segmento critico y no se sabe si sigue. Ni se cuela en la
    /// lista ni desaparece sin dejar rastro.
    #[test]
    fn un_pegajoso_expulsado_no_se_declara_presente_ni_se_pierde() {
        let mut almacen = AlmacenObservacion::nuevo();

        almacen.observar(
            MAC,
            Some(Protocolo::Hl7),
            DeclaracionSegmento::PuedeAlojarCriticos,
        );

        for numero in 0..(CAPACIDAD_VOLATIL as u32 * 2) {
            almacen.observar(
                mac_numero(numero),
                None,
                DeclaracionSegmento::SinDispositivosCriticos,
            );
        }

        assert!(
            !almacen
                .inventario()
                .iter()
                .any(|nodo| nodo.direccion == MAC),
            "expulsado del volatil, no se puede afirmar que siga en la red"
        );

        assert!(
            almacen.pegajosos_no_observados().contains(&MAC),
            "pero tampoco desaparece: se sabe que estuvo en segmento critico"
        );
    }

    /// `pegajoso` no significa «contenido», y esta prueba existe para que nadie
    /// lo lea asi dentro de un ano.
    ///
    /// El agente no contiene nada (RPT-020). La marca es resistencia a la
    /// expulsion, y derivar contencion de ella seria inventar dato justo en el
    /// campo que PA-138 se abrio para proteger.
    #[test]
    fn la_marca_pegajosa_sale_del_segmento_y_no_de_ninguna_contencion() {
        let mut almacen = AlmacenObservacion::nuevo();

        almacen.observar(MAC, None, DeclaracionSegmento::PuedeAlojarCriticos);
        almacen.observar(
            mac_numero(1),
            None,
            DeclaracionSegmento::SinDispositivosCriticos,
        );
        almacen.observar(mac_numero(2), None, DeclaracionSegmento::NoDeclarado);

        let pegajosos: Vec<DireccionEnlace> = almacen
            .inventario()
            .iter()
            .filter(|nodo| nodo.pegajoso)
            .map(|nodo| nodo.direccion)
            .collect();

        // `NoDeclarado` se trata como `PuedeAlojarCriticos`, asi que son dos.
        assert_eq!(pegajosos, vec![MAC, mac_numero(2)]);
    }

    /// Un almacen vacio es una lista vacia, no un fallo.
    #[test]
    fn un_almacen_sin_observar_nada_enumera_cero_y_no_se_queja() {
        let almacen = AlmacenObservacion::nuevo();

        assert!(almacen.inventario().is_empty());
        assert!(almacen.pegajosos_no_observados().is_empty());
    }

    #[test]
    fn la_expulsion_volatil_no_borra_la_marca_pegajosa() {
        // El hallazgo de RPT-018 §6. Sin la particion, llenar la tabla con
        // direcciones inventadas expulsaria al carro de telemedicina y volveria
        // a ser contenible.
        let mut almacen = AlmacenObservacion::nuevo();

        almacen.observar(
            MAC,
            Some(Protocolo::Hl7),
            DeclaracionSegmento::PuedeAlojarCriticos,
        );

        // El atacante rocia direcciones hasta desbordar la mitad volatil.
        for numero in 0..(CAPACIDAD_VOLATIL as u32 * 2) {
            almacen.observar(
                mac_numero(numero),
                None,
                DeclaracionSegmento::SinDispositivosCriticos,
            );
        }

        assert!(
            almacen.volatiles() <= CAPACIDAD_VOLATIL,
            "la mitad volatil debe estar acotada"
        );

        let historial = almacen
            .historial(&MAC)
            .expect("el pegajoso no esta saturado");
        assert!(
            historial.visto_en_segmento_critico,
            "la marca pegajosa debe sobrevivir a la expulsion volatil"
        );
        assert_eq!(
            historial.declaracion_efectiva(),
            DeclaracionSegmento::PuedeAlojarCriticos
        );
    }

    #[test]
    fn el_rociado_de_direcciones_no_desemboca_en_contencion() {
        // La comprobacion que importa: el efecto en el veredicto tras el ataque.
        let mut almacen = AlmacenObservacion::nuevo();
        almacen.observar(
            MAC,
            Some(Protocolo::Hl7),
            DeclaracionSegmento::PuedeAlojarCriticos,
        );
        for numero in 0..(CAPACIDAD_VOLATIL as u32 * 2) {
            almacen.observar(
                mac_numero(numero),
                None,
                DeclaracionSegmento::SinDispositivosCriticos,
            );
        }

        let proveedores = Proveedores {
            inventario: &InventarioDe(Ok(None)),
            segmento: &almacen,
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &almacen,
        };

        assert_ne!(
            evaluar(
                clasificar_con_proveedores(&proveedores, &MAC, AHORA),
                PerfilSegmento::Corporativo
            ),
            Veredicto::Ejecutar,
            "tras el rociado, el equipo clinico no puede volverse contenible"
        );
    }

    #[test]
    fn la_saturacion_del_pegajoso_bloquea_en_lugar_de_olvidar() {
        // La pregunta que RPT-018 §6 no contestaba. Llenar la mitad pegajosa
        // deja de blanquear un dispositivo y pasa a bloquear la contencion
        // entera: el atacante gana una denegacion, no un permiso.
        let mut almacen = AlmacenObservacion::nuevo();

        for numero in 0..(CAPACIDAD_PEGAJOSA as u32 + 16) {
            almacen.observar(
                mac_numero(numero),
                None,
                DeclaracionSegmento::PuedeAlojarCriticos,
            );
        }

        assert!(almacen.pegajoso_saturado());
        assert!(
            matches!(
                almacen.historial(&MAC),
                Err(ErrorProveedor::FuenteInaccesible { .. })
            ),
            "saturado, el proveedor de segmento debe fallar para todos"
        );

        // Y un fallo declarativo bloquea (RPT-010 §6).
        let proveedores = Proveedores {
            inventario: &InventarioDe(Ok(None)),
            segmento: &almacen,
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &almacen,
        };
        assert_eq!(
            clasificar_con_proveedores(&proveedores, &MAC, AHORA),
            Clasificacion::Ambiguo {
                motivo: MotivoAmbiguedad::EvidenciaNoVerificable
            }
        );
    }

    #[test]
    fn un_dispositivo_no_observado_es_indeterminado_y_no_sin_indicio() {
        // Acabar de aparecer no es haberse delatado. Colapsarlos leeria la
        // ausencia de observacion como ausencia de riesgo.
        let almacen = AlmacenObservacion::nuevo();

        assert_eq!(almacen.indicio(&MAC), Ok(Indicio::Indeterminado));
    }

    #[test]
    fn la_perdida_de_tramas_degrada_a_indeterminado() {
        // RPT-018 §4: con perdida, la huella acumulada esta incompleta y no se
        // recompone porque la perdida cese.
        let mut almacen = AlmacenObservacion::nuevo();
        almacen.observar(
            MAC,
            Some(Protocolo::Hl7),
            DeclaracionSegmento::SinDispositivosCriticos,
        );
        assert_eq!(
            almacen.indicio(&MAC),
            Ok(Indicio::SugiereCriticidad(ClaseExcluida::SoporteVital))
        );

        almacen.anotar_perdida();
        assert_eq!(almacen.indicio(&MAC), Ok(Indicio::Indeterminado));
    }

    #[test]
    fn un_protocolo_observado_puede_no_sugerir_nada() {
        // BACnet automatiza edificios: su fallo importa, pero no es soporte
        // vital. Sirve para comprobar que el almacen distingue «observado» de
        // «indicativo» en lugar de tratar toda observacion como sospecha.
        let mut almacen = AlmacenObservacion::nuevo();
        almacen.observar(
            MAC,
            Some(Protocolo::Bacnet),
            DeclaracionSegmento::SinDispositivosCriticos,
        );

        assert_eq!(almacen.indicio(&MAC), Ok(Indicio::SinIndicio));
        assert_eq!(Protocolo::Bacnet.sugiere(), None);
    }

    #[test]
    fn los_protocolos_industriales_sugieren_seguridad_funcional() {
        assert_eq!(
            Protocolo::Modbus.sugiere(),
            Some(ClaseExcluida::SeguridadFuncional)
        );
        assert_eq!(
            Protocolo::Dnp3.sugiere(),
            Some(ClaseExcluida::SeguridadFuncional)
        );
        assert_eq!(Protocolo::Hl7.sugiere(), Some(ClaseExcluida::SoporteVital));
    }

    #[test]
    fn el_mismo_almacen_sirve_a_las_dos_fuentes() {
        // RPT-018 §8.3 anticipaba que huella y segmento podrian dejar de ser
        // independientes. Lo son: la VLAN se conoce por la misma observacion que
        // el protocolo, y mantener dos tablas con las mismas direcciones es como
        // se desincronizan.
        let mut almacen = AlmacenObservacion::nuevo();
        almacen.observar(
            MAC,
            Some(Protocolo::Modbus),
            DeclaracionSegmento::PuedeAlojarCriticos,
        );

        assert_eq!(
            almacen.indicio(&MAC),
            Ok(Indicio::SugiereCriticidad(
                ClaseExcluida::SeguridadFuncional
            ))
        );
        assert_eq!(
            almacen.historial(&MAC).expect("no saturado").actual,
            DeclaracionSegmento::PuedeAlojarCriticos
        );
    }

    // -----------------------------------------------------------------------
    // Revocacion — RPT-015, PA-33
    // -----------------------------------------------------------------------

    use revocacion::{
        Anotacion, ArchivoRevocaciones, CertificadoRevocacion, CertificadoVerificado, ErrorArchivo,
        ErrorRevocacion, IdentificadorClave, RegistroRevocaciones, mensaje_de_certificado,
    };

    /// Par de recuperacion, distinto del operativo por semilla.
    fn clave_de_recuperacion(dominio: DominioClave) -> (ClaveFirmaHibrida, ClaveInventario) {
        let (firmante, verificadora) = generar_par(&mut GeneradorDeterminista::nuevo(0x52_45_43));
        (firmante, ClaveInventario::nueva(verificadora, dominio))
    }

    /// Firma un certificado con la clave dada.
    fn firmar_certificado(
        certificado: &CertificadoRevocacion,
        firmante: &ClaveFirmaHibrida,
    ) -> FirmaHibrida {
        motor_pqc::firma_hibrida::firmar(firmante, &mensaje_de_certificado(certificado))
    }

    /// Certificado que revoca la clave operativa del banco a partir del corte.
    fn certificado_para(banco: &Banco, corte: u64) -> CertificadoRevocacion {
        let (_, sucesora) = generar_par(&mut GeneradorDeterminista::nuevo(SEMILLA_SUCESORA));
        CertificadoRevocacion {
            revocada: banco.clave.identificador(),
            hasta_secuencia: corte,
            sucesora: IdentificadorClave::de(&sucesora),
            emitido_en: AHORA,
        }
    }

    #[test]
    fn un_certificado_valido_verifica() {
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let certificado = certificado_para(&banco, 5);

        let verificado = CertificadoVerificado::verificar(
            certificado,
            &firmar_certificado(&certificado, &firmante),
            &clave,
        )
        .expect("la cadena esta completa");

        assert_eq!(verificado.hasta_secuencia(), 5);
        assert_eq!(verificado.revocada(), banco.clave.identificador());
    }

    #[test]
    fn la_clave_operativa_no_puede_revocarse_a_si_misma() {
        // Es el control central del §4: si la operativa pudiera firmar
        // certificados, quien la roba se «autorrevocaria» a un corte alto, que es
        // lo contrario de una revocacion.
        let banco = banco(DominioClave::Cliente);
        let (firmante, _) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let certificado = certificado_para(&banco, 5);

        assert_eq!(
            CertificadoVerificado::verificar(
                certificado,
                &firmar_certificado(&certificado, &firmante),
                &banco.clave,
            ),
            Err(ErrorRevocacion::DominioDeClaveIncorrecto {
                encontrado: DominioClave::Cliente,
            })
        );
    }

    #[test]
    fn la_clave_de_premoscorp_tampoco_revoca() {
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::PremosCorp);
        let certificado = certificado_para(&banco, 5);

        assert_eq!(
            CertificadoVerificado::verificar(
                certificado,
                &firmar_certificado(&certificado, &firmante),
                &clave,
            ),
            Err(ErrorRevocacion::DominioDeClaveIncorrecto {
                encontrado: DominioClave::PremosCorp,
            })
        );
    }

    #[test]
    fn un_certificado_que_se_revoca_a_si_mismo_se_rechaza() {
        // Declarar la misma clave como revocada y como sucesora dejaria al
        // cliente sin autoridad ninguna sobre su inventario.
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        let certificado = CertificadoRevocacion {
            revocada: banco.clave.identificador(),
            hasta_secuencia: 5,
            sucesora: banco.clave.identificador(),
            emitido_en: AHORA,
        };

        assert_eq!(
            CertificadoVerificado::verificar(
                certificado,
                &firmar_certificado(&certificado, &firmante),
                &clave,
            ),
            Err(ErrorRevocacion::SucesoraEsLaRevocada)
        );
    }

    #[test]
    fn alterar_el_certificado_invalida_su_firma() {
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        let original = certificado_para(&banco, 5);
        let firma = firmar_certificado(&original, &firmante);

        // Subir el corte es la alteracion util: afloja la revocacion.
        let mut alterado = original;
        alterado.hasta_secuencia = u64::MAX;

        assert_eq!(
            CertificadoVerificado::verificar(alterado, &firma, &clave),
            Err(ErrorRevocacion::FirmaInvalida)
        );
    }

    // --- El sexto eslabon ---

    #[test]
    fn una_clave_revocada_no_firma_por_encima_de_su_corte() {
        let mut banco = banco_con(DominioClave::Cliente, 9, Centinela::Establecido(7));
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let certificado = certificado_para(&banco, 7);

        let verificado = CertificadoVerificado::verificar(
            certificado,
            &firmar_certificado(&certificado, &firmante),
            &clave,
        )
        .expect("certificado valido");

        banco.revocaciones.anotar(&verificado);

        assert_eq!(
            banco.raiz_verificada(),
            Err(ErrorInventario::ClaveRevocada {
                presentada: 9,
                corte: 7,
            })
        );
    }

    #[test]
    fn una_clave_revocada_sigue_valiendo_por_debajo_del_corte() {
        // El §3: revocar de golpe dejaria al agente sin inventario, y sin
        // marcados los equipos criticos dejan de estar protegidos.
        let mut banco = banco_con(DominioClave::Cliente, 5, Centinela::Establecido(5));
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let certificado = certificado_para(&banco, 7);

        let verificado = CertificadoVerificado::verificar(
            certificado,
            &firmar_certificado(&certificado, &firmante),
            &clave,
        )
        .expect("certificado valido");

        banco.revocaciones.anotar(&verificado);

        assert!(
            banco.raiz_verificada().is_ok(),
            "un inventario anterior al corte debe seguir sirviendo"
        );
    }

    #[test]
    fn el_registro_conserva_el_corte_mas_bajo() {
        // Una revocacion que se puede aflojar no es una revocacion.
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let mut registro = RegistroRevocaciones::nuevo();

        for corte in [7, 20, 3, 15] {
            let certificado = certificado_para(&banco, corte);
            let verificado = CertificadoVerificado::verificar(
                certificado,
                &firmar_certificado(&certificado, &firmante),
                &clave,
            )
            .expect("certificado valido");
            registro.anotar(&verificado);
        }

        assert_eq!(registro.anotadas(), 1, "es la misma clave, no cuatro");
        assert_eq!(registro.corte_de(&banco.clave.identificador()), Some(3));
    }

    #[test]
    fn una_clave_no_revocada_firma_cualquier_secuencia() {
        let registro = RegistroRevocaciones::nuevo();
        let banco = banco(DominioClave::Cliente);

        assert!(registro.admite(&banco.clave.identificador(), u64::MAX));
    }

    // --- PA-33: el bloqueo por saturacion de secuencia ---

    #[test]
    fn el_bloqueo_por_secuencia_maxima_se_recupera_con_el_certificado() {
        // El ataque de un solo mensaje. Sin el reinicio del centinela, el
        // atacante emite u64::MAX, pierde la clave al revocarse, y a cambio deja
        // el inventario congelado de forma irreversible: peor que el compromiso.
        let (firmante, clave_recuperacion) =
            clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        // 1. El atacante satura la secuencia y el centinela sube al maximo.
        let saturado = banco_con(DominioClave::Cliente, u64::MAX, Centinela::Establecido(9));
        assert!(saturado.raiz_verificada().is_ok(), "la firma es valida");
        let centinela_saturado = saturado.centinela.avanzar(u64::MAX);
        assert_eq!(centinela_saturado, Centinela::Establecido(u64::MAX));

        // 2. Sin certificado, ningun inventario legitimo puede superarlo.
        let legitimo = banco_con(DominioClave::Cliente, 10, centinela_saturado);
        assert!(
            matches!(
                legitimo.raiz_verificada(),
                Err(ErrorInventario::ReversionDetectada { .. })
            ),
            "el bloqueo debe ser real antes de comprobar que se sale de el"
        );

        // 3. El certificado corta en 9 y reinicia el centinela.
        let certificado = certificado_para(&saturado, 9);
        let verificado = CertificadoVerificado::verificar(
            certificado,
            &firmar_certificado(&certificado, &firmante),
            &clave_recuperacion,
        )
        .expect("certificado valido");

        let centinela = centinela_saturado.reiniciar_por(&verificado);
        assert_eq!(centinela, Centinela::Establecido(9));

        // 4. El sucesor vuelve a entrar. Firma con la clave NUEVA: la revocada ya
        //    no admite nada por encima de 9, asi que sin rotar no se sale del
        //    bloqueo.
        let mut sucesor = banco_con_semilla(DominioClave::Cliente, 10, centinela, SEMILLA_SUCESORA);
        sucesor.revocaciones.anotar(&verificado);
        assert_eq!(
            sucesor.clave.identificador(),
            verificado.sucesora(),
            "el banco sucesor debe usar la clave que el certificado nombra"
        );

        assert!(
            sucesor.raiz_verificada().is_ok(),
            "tras la revocacion el cliente debe recuperar el control"
        );

        // Y la clave vieja sigue sin poder emitir por encima de su corte.
        let mut reintento = banco_con(DominioClave::Cliente, 10, centinela);
        reintento.revocaciones.anotar(&verificado);
        assert_eq!(
            reintento.raiz_verificada(),
            Err(ErrorInventario::ClaveRevocada {
                presentada: 10,
                corte: 9,
            })
        );
    }

    // --- PA-34: persistencia del registro ---

    /// Anotacion firmada lista para persistir.
    fn anotacion_para(banco: &Banco, corte: u64, firmante: &ClaveFirmaHibrida) -> Anotacion {
        let certificado = certificado_para(banco, corte);
        Anotacion {
            firma: firmar_certificado(&certificado, firmante),
            certificado,
        }
    }

    #[test]
    fn el_archivo_de_revocaciones_es_reversible() {
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        let mut archivo = ArchivoRevocaciones::nuevo();
        archivo.anotar(anotacion_para(&banco, 7, &firmante));

        let bytes = archivo.serializar();
        let leido = ArchivoRevocaciones::analizar(&bytes, &clave).expect("debe reverificar");

        assert_eq!(leido.anotaciones().len(), 1);
        assert_eq!(leido.serializar(), bytes);
        assert_eq!(
            leido.registro().corte_de(&banco.clave.identificador()),
            Some(7)
        );
    }

    #[test]
    fn alterar_un_corte_en_el_fichero_se_detecta() {
        // El motivo de guardar la firma y no solo el par derivado. Sin
        // reverificar, subir el corte aflojaria la revocacion sin dejar rastro.
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        let mut archivo = ArchivoRevocaciones::nuevo();
        archivo.anotar(anotacion_para(&banco, 7, &firmante));

        let mut bytes = archivo.serializar();
        // El corte vive tras la cabecera (14) y el identificador revocado (32).
        bytes[14 + 32..14 + 40].copy_from_slice(&u64::MAX.to_be_bytes());

        assert!(matches!(
            ArchivoRevocaciones::analizar(&bytes, &clave),
            Err(ErrorArchivo::NoVerifica { posicion: 0, .. })
        ));
    }

    #[test]
    fn un_archivo_sin_anotaciones_es_valido() {
        // A diferencia del inventario, un registro de revocaciones vacio es el
        // estado normal: la mayoria de los despliegues nunca revocan nada.
        let (_, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let bytes = ArchivoRevocaciones::nuevo().serializar();

        let leido = ArchivoRevocaciones::analizar(&bytes, &clave).expect("vacio es valido");
        assert_eq!(leido.anotaciones().len(), 0);
        assert_eq!(leido.registro().anotadas(), 0);
    }

    #[test]
    fn el_archivo_conserva_el_corte_mas_bajo() {
        let banco = banco(DominioClave::Cliente);
        let (firmante, _) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        let mut archivo = ArchivoRevocaciones::nuevo();
        for corte in [9, 3, 20] {
            archivo.anotar(anotacion_para(&banco, corte, &firmante));
        }

        assert_eq!(archivo.anotaciones().len(), 1, "es la misma clave");
        assert_eq!(
            archivo.registro().corte_de(&banco.clave.identificador()),
            Some(3)
        );
    }

    #[test]
    fn un_magico_ajeno_en_revocaciones_se_rechaza() {
        let (_, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let mut bytes = ArchivoRevocaciones::nuevo().serializar();
        bytes[0] = b'X';

        assert_eq!(
            ArchivoRevocaciones::analizar(&bytes, &clave).err(),
            Some(ErrorArchivo::MagicoAusente)
        );
    }

    #[test]
    fn bytes_sobrantes_en_revocaciones_se_rechazan() {
        let (_, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let mut bytes = ArchivoRevocaciones::nuevo().serializar();
        bytes.extend_from_slice(b"cola");

        assert_eq!(
            ArchivoRevocaciones::analizar(&bytes, &clave).err(),
            Some(ErrorArchivo::BytesSobrantes { sobrantes: 4 })
        );
    }

    #[test]
    fn un_numero_de_anotaciones_absurdo_no_reserva_memoria() {
        let (_, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);
        let mut bytes = Vec::from(*revocacion::MAGICO_REVOCACIONES);
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&u32::MAX.to_be_bytes());

        assert_eq!(
            ArchivoRevocaciones::analizar(&bytes, &clave).err(),
            Some(ErrorArchivo::DemasiadasAnotaciones {
                declaradas: u32::MAX as usize
            })
        );
    }

    #[test]
    fn el_registro_sobrevive_a_un_ciclo_por_disco() {
        // PA-34 de extremo a extremo: la revocacion deja de durar lo que dura el
        // proceso.
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        let mut archivo = ArchivoRevocaciones::nuevo();
        archivo.anotar(anotacion_para(&banco, 4, &firmante));

        let directorio = std::env::temp_dir().join("eje-latam-revocaciones");
        let _ = std::fs::remove_dir_all(&directorio);
        std::fs::create_dir_all(&directorio).expect("directorio de prueba");
        let ruta = directorio.join("revocaciones.rev");

        disco::escribir_atomico(&ruta, &archivo.serializar()).expect("escritura atomica");
        let leido = ArchivoRevocaciones::analizar(&disco::leer(&ruta).expect("lectura"), &clave)
            .expect("reverificacion");

        assert_eq!(
            leido.registro().corte_de(&banco.clave.identificador()),
            Some(4)
        );

        let _ = std::fs::remove_dir_all(&directorio);
    }

    // -----------------------------------------------------------------------
    // Arranque y persistencia — RPT-017, PA-35
    // -----------------------------------------------------------------------

    use std::path::PathBuf;

    use arranque::{
        Centinelas, ErrorArranque, EstadoArranque, RutasAlmacen, aceptar_configuracion,
        aceptar_inventario, analizar_centinela, arrancar, cargar_centinela, serializar_centinela,
    };

    /// Almacen aislado en disco, retirado al terminar.
    struct AlmacenDePrueba {
        rutas: RutasAlmacen,
    }

    impl AlmacenDePrueba {
        fn nuevo(nombre: &str) -> Self {
            let directorio = std::env::temp_dir().join(format!("eje-latam-arranque-{nombre}"));
            let _ = std::fs::remove_dir_all(&directorio);
            std::fs::create_dir_all(&directorio).expect("directorio de prueba");
            Self {
                rutas: RutasAlmacen::nuevo(directorio),
            }
        }
    }

    impl Drop for AlmacenDePrueba {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.rutas.directorio());
        }
    }

    /// Clave de recuperación con la que se leen las revocaciones al arrancar.
    fn clave_recuperacion_de_prueba() -> ClaveInventario {
        clave_de_recuperacion(DominioClave::ClienteRecuperacion).1
    }

    /// Un centinela con las dos marcas puestas, para las pruebas de formato.
    fn centinelas_de_prueba() -> Centinelas {
        Centinelas {
            inventario: Centinela::Establecido(42),
            configuracion: Centinela::Establecido(3),
        }
    }

    #[test]
    fn el_centinela_es_reversible_en_disco() {
        assert_eq!(
            analizar_centinela(&serializar_centinela(centinelas_de_prueba()))
                .expect("debe analizar"),
            centinelas_de_prueba()
        );
    }

    /// Y cada marca va por su lado. RPT-078, PA-79 paso 5.
    ///
    /// Las cuatro combinaciones, porque el fallo que importa es el asimetrico:
    /// un fichero que dice «inventario si, configuracion no» tiene que volver
    /// diciendo exactamente eso. Si las dos marcas se contagiaran, aceptar un
    /// inventario borraria la marca de configuracion y cualquier configuracion
    /// antigua entraria.
    #[test]
    fn las_dos_marcas_del_centinela_no_se_contagian() {
        for (inventario, configuracion) in [
            (Centinela::Establecido(9), Centinela::Establecido(4)),
            (Centinela::Establecido(9), Centinela::SinEstablecer),
            (Centinela::SinEstablecer, Centinela::Establecido(4)),
        ] {
            let centinelas = Centinelas {
                inventario,
                configuracion,
            };

            assert_eq!(
                analizar_centinela(&serializar_centinela(centinelas)).expect("debe analizar"),
                centinelas,
                "las marcas se pisan al ir y volver del disco"
            );
        }
    }

    /// Cero de presencia significa cero de valor, y no hay segunda forma.
    ///
    /// RPT-078. Una marca «ausente» con un valor escrito encima seria una segunda
    /// codificacion de «sin establecer», y la segunda codificacion es siempre por
    /// donde se cuela algo: bastaria para escribir un centinela que el analizador
    /// lee de una forma y el que lo compuso creia de otra.
    #[test]
    fn una_marca_ausente_con_valor_encima_se_rechaza() {
        let mut bytes = serializar_centinela(Centinelas {
            inventario: Centinela::Establecido(9),
            configuracion: Centinela::SinEstablecer,
        });
        // La marca de configuracion empieza en 19: presencia en 19, valor en 20.
        bytes[27] = 5;

        assert!(matches!(
            analizar_centinela(&bytes),
            Err(ErrorArranque::CentinelaCorrupto)
        ));
    }

    #[test]
    fn un_centinela_corrupto_no_se_degrada_a_primer_arranque() {
        // Es el ataque del §2 por otra puerta: si corromper el fichero simulara
        // un primer arranque, borrar el inventario volveria a funcionar.
        let casos: Vec<(Vec<u8>, &str)> = vec![
            (vec![], "vacio"),
            (vec![0u8; 28], "ceros con la longitud correcta"),
            (
                serializar_centinela(centinelas_de_prueba())[..27].to_vec(),
                "truncado por un byte",
            ),
            (
                {
                    let mut bytes = serializar_centinela(centinelas_de_prueba());
                    bytes[9] = 9;
                    bytes
                },
                "version desconocida",
            ),
            (
                {
                    // RPT-078. La version 1 llevaba UNA marca. Aceptarla ahora
                    // seria admitir un fichero que no dice nada de la secuencia
                    // de configuracion: escribir dieciocho bytes a mano dejaria
                    // entrar cualquier configuracion antigua y bien firmada. La
                    // compatibilidad hacia atras seria aqui la vulnerabilidad.
                    let mut bytes = arranque::MAGICO_CENTINELA.to_vec();
                    bytes.extend_from_slice(&1u16.to_be_bytes());
                    bytes.extend_from_slice(&42u64.to_be_bytes());
                    bytes
                },
                "el formato anterior, de una sola marca",
            ),
            (
                {
                    // Un fichero que no afirma ninguna de las dos series. Eso ya
                    // lo dice la AUSENCIA del fichero, y admitir las dos formas
                    // dejaria poner un centinela que no dice nada donde habia uno
                    // que decia algo.
                    let mut bytes = arranque::MAGICO_CENTINELA.to_vec();
                    bytes.extend_from_slice(&arranque::VERSION_CENTINELA.to_be_bytes());
                    bytes.extend_from_slice(&[0u8; 18]);
                    bytes
                },
                "presente y sin afirmar nada",
            ),
            (
                {
                    let mut bytes = serializar_centinela(centinelas_de_prueba());
                    bytes[10] = 2; // presencia que no es ni 0 ni 1
                    bytes
                },
                "bandera de presencia fuera del enumerado",
            ),
        ];

        for (caso, que) in casos {
            assert!(
                matches!(
                    analizar_centinela(&caso),
                    Err(ErrorArranque::CentinelaCorrupto)
                ),
                "un centinela {que} no debe degradarse en silencio"
            );
        }
    }

    #[test]
    fn un_almacen_vacio_es_un_primer_arranque() {
        let almacen = AlmacenDePrueba::nuevo("vacio");
        let banco = banco(DominioClave::Cliente);

        let (estado, centinela) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("un almacen vacio debe dejar arrancar");

        assert!(matches!(estado, EstadoArranque::PrimerArranque));
        assert_eq!(centinela, Centinelas::SIN_ESTABLECER);
        assert!(estado.admite_contencion_automatica());
        assert!(
            !estado.exige_alerta(),
            "un primer arranque es normal y no debe alertar"
        );
        assert_eq!(estado.marcado(&MAC), Ok(None));
    }

    #[test]
    fn borrar_el_inventario_se_detecta_como_supresion() {
        // El ataque del §2. Sin el testigo del centinela, esto seria
        // indistinguible de un primer arranque y el equipo de soporte vital
        // pasaria a ser contenible.
        let almacen = AlmacenDePrueba::nuevo("supresion");
        let banco = banco(DominioClave::Cliente);

        aceptar_inventario(&almacen.rutas, &bytes_en_disco(&banco), 7).expect("aceptar");
        std::fs::remove_file(almacen.rutas.inventario()).expect("el atacante borra el fichero");

        let (estado, _) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("el agente debe arrancar igual");

        assert!(matches!(
            estado,
            EstadoArranque::Supresion {
                secuencia_conocida: 7
            }
        ));
        assert!(
            !estado.admite_contencion_automatica(),
            "tras una supresion no puede haber contencion automatica"
        );
        assert!(estado.exige_alerta());

        // Y el proveedor devuelve error, no ausencia: la clasificacion resuelve
        // en ambiguedad y no en «contenible».
        assert!(matches!(
            estado.marcado(&MAC),
            Err(ErrorProveedor::FirmaInvalida { .. })
        ));
    }

    #[test]
    fn la_supresion_no_desemboca_en_contencion() {
        // La comprobacion que importa de verdad: el efecto en el veredicto.
        let almacen = AlmacenDePrueba::nuevo("supresion-veredicto");
        let banco = banco(DominioClave::Cliente);

        aceptar_inventario(&almacen.rutas, &bytes_en_disco(&banco), 7).expect("aceptar");
        std::fs::remove_file(almacen.rutas.inventario()).expect("borrado");

        let (estado, _) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("arranque");

        let proveedores = Proveedores {
            inventario: &estado,
            // Segmento declarado limpio: es el caso en el que el borrado
            // convertiria un equipo critico en contenible.
            segmento: &SegmentoDe(Ok(segmento_limpio())),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SinIndicio)),
        };

        let veredicto = evaluar(
            clasificar_con_proveedores(&proveedores, &MAC, AHORA),
            PerfilSegmento::Corporativo,
        );

        assert_ne!(veredicto, Veredicto::Ejecutar);
        assert!(veredicto.exige_alerta());
    }

    #[test]
    fn un_primer_arranque_si_permite_contener_en_segmento_limpio() {
        // La otra mitad: si el primer arranque bloqueara todo, el producto no
        // contendria nada nunca en una instalacion nueva. Es la paralisis que
        // RPT-009 §5 resolvio.
        let almacen = AlmacenDePrueba::nuevo("primer-arranque-contiene");
        let banco = banco(DominioClave::Cliente);

        let (estado, _) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("arranque");

        let proveedores = Proveedores {
            inventario: &estado,
            segmento: &SegmentoDe(Ok(segmento_limpio())),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SinIndicio)),
        };

        assert_eq!(
            evaluar(
                clasificar_con_proveedores(&proveedores, &MAC, AHORA),
                PerfilSegmento::Corporativo
            ),
            Veredicto::Ejecutar
        );
    }

    #[test]
    fn un_inventario_alterado_en_disco_arranca_pero_no_verifica() {
        let almacen = AlmacenDePrueba::nuevo("alterado");
        let banco = banco(DominioClave::Cliente);

        let mut bytes = bytes_en_disco(&banco);
        aceptar_inventario(&almacen.rutas, &bytes, 7).expect("aceptar");

        // Degradar la primera entrada a «no critico», como en RPT-013.
        let posicion_clase = CABECERA + banco.posicion(&MAC) * ENTRADA + 6;
        bytes[posicion_clase] = 0;
        std::fs::write(almacen.rutas.inventario(), &bytes).expect("el atacante reescribe");

        let (estado, _) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("el agente arranca igual");

        assert!(matches!(estado, EstadoArranque::NoVerifica { .. }));
        assert!(!estado.admite_contencion_automatica());
        assert!(estado.exige_alerta());
    }

    #[test]
    fn un_formato_anterior_no_se_confunde_con_manipulacion() {
        // El defecto que este estado existe para deshacer: con la comprobacion
        // anterior, subir VERSION convertia CADA actualizacion rutinaria del
        // agente en una alerta maxima de ataque, en todos los sitios a la vez.
        //
        // La fatiga de alertas que eso produce cuesta mas que reemitir: un
        // operador que aprendio a ignorar esta alerta la ignorara tambien el dia
        // que sea cierta.
        let almacen = AlmacenDePrueba::nuevo("formato-obsoleto");
        let banco = banco(DominioClave::Cliente);

        let mut bytes = bytes_en_disco(&banco);
        aceptar_inventario(&almacen.rutas, &bytes, 7).expect("aceptar");

        // El fichero se emitio con la version anterior del formato.
        bytes[8..10].copy_from_slice(&(formato::VERSION - 1).to_be_bytes());
        std::fs::write(almacen.rutas.inventario(), &bytes).expect("reescribir");

        let (estado, _) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("el agente arranca igual");

        assert!(matches!(estado, EstadoArranque::FormatoObsoleto { .. }));
        assert!(
            estado.exige_alerta(),
            "hay que avisar: nadie mas va a recordar que toca reemitir"
        );
        assert!(
            !estado.es_manipulacion(),
            "pero NO es un ataque, y presentarlo como tal produce fatiga de alertas"
        );
    }

    #[test]
    fn un_formato_obsoleto_protege_como_el_primer_arranque() {
        // Sin marcados legibles, la clasificacion resuelve por segmento. No hay
        // razon para bloquear tambien la contencion donde el administrador ya
        // declaro que no hay criticos.
        let almacen = AlmacenDePrueba::nuevo("obsoleto-protege");
        let banco = banco(DominioClave::Cliente);

        let mut bytes = bytes_en_disco(&banco);
        aceptar_inventario(&almacen.rutas, &bytes, 7).expect("aceptar");
        bytes[8..10].copy_from_slice(&(formato::VERSION - 1).to_be_bytes());
        std::fs::write(almacen.rutas.inventario(), &bytes).expect("reescribir");

        let (estado, _) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("arranque");

        assert!(estado.admite_contencion_automatica());
        assert_eq!(
            estado.marcado(&MAC),
            Ok(None),
            "sin marcados legibles, pero ausencia legitima y no error"
        );

        // Y en un segmento que admite criticos sigue sin contenerse.
        let proveedores = Proveedores {
            inventario: &estado,
            segmento: &SegmentoDe(Ok(HistorialSegmento {
                actual: DeclaracionSegmento::PuedeAlojarCriticos,
                visto_en_segmento_critico: false,
            })),
            oui: &OuiDe(Ok(Indicio::SinIndicio)),
            huella: &HuellaDe(Ok(Indicio::SinIndicio)),
        };

        assert_ne!(
            evaluar(
                clasificar_con_proveedores(&proveedores, &MAC, AHORA),
                PerfilSegmento::Corporativo
            ),
            Veredicto::Ejecutar
        );
    }

    #[test]
    fn una_version_posterior_sigue_siendo_desconocida() {
        // La distincion es en las dos direcciones: hacia atras es caducidad,
        // hacia delante es un formato que no se puede interpretar.
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);
        bytes[8..10].copy_from_slice(&(formato::VERSION + 1).to_be_bytes());

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::VersionDesconocida {
                encontrada: formato::VERSION + 1
            })
        );
    }

    #[test]
    fn una_version_anterior_es_obsoleta_y_no_desconocida() {
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);
        bytes[8..10].copy_from_slice(&(formato::VERSION - 1).to_be_bytes());

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::FormatoObsoleto {
                encontrada: formato::VERSION - 1
            })
        );
    }

    // -----------------------------------------------------------------------
    // Clave aprovisionada — RPT-024, PA-49
    // -----------------------------------------------------------------------

    use arranque::{aprovisionar_clave, arrancar_con_almacen, cargar_clave};
    use clave::{
        ErrorClave, MAGICO_CLAVE, VERSION_CLAVE, analizar as analizar_clave, longitud_fichero,
        serializar as serializar_clave,
    };

    /// Aprovisiona el par de claves del banco en el almacen de prueba.
    fn aprovisionar_claves(almacen: &AlmacenDePrueba, banco: &Banco) {
        aprovisionar_clave(
            &almacen.rutas.clave_operativa(),
            banco.clave.verificacion(),
            DominioClave::Cliente,
        )
        .expect("aprovisionar operativa");

        aprovisionar_clave(
            &almacen.rutas.clave_recuperacion(),
            clave_recuperacion_de_prueba().verificacion(),
            DominioClave::ClienteRecuperacion,
        )
        .expect("aprovisionar recuperacion");
    }

    #[test]
    fn el_fichero_de_clave_es_reversible() {
        let banco = banco(DominioClave::Cliente);
        let bytes = serializar_clave(banco.clave.verificacion(), DominioClave::Cliente);

        assert_eq!(bytes.len(), longitud_fichero());

        let leida = analizar_clave(&bytes, DominioClave::Cliente).expect("debe analizar");

        assert_eq!(leida.dominio(), DominioClave::Cliente);
        assert_eq!(
            leida.verificacion().a_bytes(),
            banco.clave.verificacion().a_bytes()
        );
        assert_eq!(leida.identificador(), banco.clave.identificador());
    }

    #[test]
    fn el_dominio_viaja_en_el_fichero_y_no_en_la_ruta() {
        // Es lo que sostiene la separacion de RPT-011 §4. Si el dominio se
        // dedujera de la ruta, colocar el fichero de recuperacion donde va el
        // operativo dejaria que la clave de emergencia firmara inventarios
        // corrientes.
        let recuperacion = clave_recuperacion_de_prueba();
        let bytes = serializar_clave(
            recuperacion.verificacion(),
            DominioClave::ClienteRecuperacion,
        );

        assert_eq!(
            analizar_clave(&bytes, DominioClave::Cliente).err(),
            Some(ErrorClave::DominioInesperado {
                encontrado: DominioClave::ClienteRecuperacion,
                esperado: DominioClave::Cliente,
            }),
            "se rechaza por lo que ES, no por donde esta"
        );
    }

    #[test]
    fn el_analizador_de_clave_resiste_entrada_hostil() {
        let banco = banco(DominioClave::Cliente);
        let valido = serializar_clave(banco.clave.verificacion(), DominioClave::Cliente);

        // Longitudes cortas: ninguna debe desbordar.
        for longitud in 0..40 {
            assert!(analizar_clave(&vec![0u8; longitud], DominioClave::Cliente).is_err());
        }

        let mut ajeno = valido.clone();
        ajeno[0] = b'X';
        assert_eq!(
            analizar_clave(&ajeno, DominioClave::Cliente).err(),
            Some(ErrorClave::MagicoAusente)
        );

        let mut futuro = valido.clone();
        futuro[8..10].copy_from_slice(&(VERSION_CLAVE + 1).to_be_bytes());
        assert_eq!(
            analizar_clave(&futuro, DominioClave::Cliente).err(),
            Some(ErrorClave::VersionDesconocida {
                encontrada: VERSION_CLAVE + 1
            })
        );

        // El cero de dominio no se interpreta: un fichero de ceros no es clave.
        let mut sin_dominio = valido.clone();
        sin_dominio[10] = 0;
        assert_eq!(
            analizar_clave(&sin_dominio, DominioClave::Cliente).err(),
            Some(ErrorClave::DominioDesconocido { codigo: 0 })
        );

        // Cola sobrante: dos lecturas del mismo fichero.
        let mut con_cola = valido.clone();
        con_cola.extend_from_slice(b"cola");
        assert!(matches!(
            analizar_clave(&con_cola, DominioClave::Cliente),
            Err(ErrorClave::LongitudIncorrecta { .. })
        ));

        // Material alterado. Un byte cambiado en la componente clasica puede
        // dar un punto invalido —y entonces se rechaza— o un punto valido, y
        // entonces es OTRA clave. Lo que no puede pasar es que se lea como la
        // misma: la aserción es sobre el identificador, no sobre el error.
        let mut alterada = valido.clone();
        let ultimo = alterada.len() - 1;
        alterada[ultimo] ^= 0xFF;

        if let Ok(leida) = analizar_clave(&alterada, DominioClave::Cliente) {
            assert_ne!(
                leida.identificador(),
                banco.clave.identificador(),
                "un byte alterado no puede producir la misma clave"
            );
        }

        assert_eq!(&valido[..8], MAGICO_CLAVE);
    }

    #[test]
    fn el_agente_carga_su_propia_clave_y_verifica_el_inventario() {
        // PA-49 completo. Antes de esto `arrancar` exigia dos claves que nadie
        // le daba, asi que el sensor operaba en primer arranque permanente y un
        // manifiesto firmado no servia de nada.
        let almacen = AlmacenDePrueba::nuevo("clave-aprovisionada");
        let banco = banco(DominioClave::Cliente);

        aprovisionar_claves(&almacen, &banco);
        aceptar_inventario(&almacen.rutas, &bytes_en_disco(&banco), 7).expect("aceptar");

        let (estado, _) = arrancar_con_almacen(&almacen.rutas).expect("arranque");

        assert!(matches!(estado, EstadoArranque::Operativo(_)));
        assert!(!estado.exige_alerta());
        assert_eq!(
            estado
                .marcado(&MAC)
                .expect("verifica")
                .expect("la mac figura")
                .clase(),
            Some(ClaseExcluida::SoporteVital),
            "el marcado llega verificado desde disco, sin clave pasada a mano"
        );
    }

    #[test]
    fn sin_clave_no_se_finge_una_instalacion_completa() {
        // Colapsarlo en `PrimerArranque` diria que la instalacion esta lista
        // cuando le falta la mitad, y el administrador no se enteraria hasta
        // emitir un manifiesto que el agente ignora en silencio.
        let almacen = AlmacenDePrueba::nuevo("sin-clave");

        let (estado, _) = arrancar_con_almacen(&almacen.rutas).expect("arranque");

        assert!(matches!(estado, EstadoArranque::SinClaveAprovisionada));
        assert!(
            estado.exige_alerta(),
            "hay que avisar de que falta la mitad"
        );
        assert!(!estado.es_manipulacion(), "pero no es un ataque");
        assert!(
            estado.admite_contencion_automatica(),
            "sin marcados, la clasificacion resuelve por segmento"
        );
        assert_eq!(estado.marcado(&MAC), Ok(None));
    }

    #[test]
    fn borrar_la_clave_despues_de_aprovisionar_es_supresion() {
        // El centinela sigue siendo el testigo. Si alguna vez se acepto un
        // inventario, hubo clave; que ya no este no es una instalacion a medias.
        let almacen = AlmacenDePrueba::nuevo("clave-borrada");
        let banco = banco(DominioClave::Cliente);

        aprovisionar_claves(&almacen, &banco);
        aceptar_inventario(&almacen.rutas, &bytes_en_disco(&banco), 7).expect("aceptar");
        std::fs::remove_file(almacen.rutas.clave_operativa()).expect("borrar la clave");

        let (estado, _) = arrancar_con_almacen(&almacen.rutas).expect("arranque");

        assert!(matches!(
            estado,
            EstadoArranque::Supresion {
                secuencia_conocida: 7
            }
        ));
        assert!(estado.es_manipulacion());
        assert!(
            !estado.admite_contencion_automatica(),
            "borrar la clave no puede volver contenible a un equipo protegido"
        );
    }

    #[test]
    fn una_clave_corrupta_no_se_degrada_a_ausente() {
        // Mismo argumento que el centinela corrupto: corromper el fichero seria
        // una via para simular el estado que exime de verificar.
        let almacen = AlmacenDePrueba::nuevo("clave-corrupta");
        let banco = banco(DominioClave::Cliente);

        aprovisionar_claves(&almacen, &banco);
        std::fs::write(almacen.rutas.clave_operativa(), b"no soy una clave").expect("corromper");

        assert!(
            arrancar_con_almacen(&almacen.rutas).is_err(),
            "no se degrada a SinClaveAprovisionada"
        );
    }

    #[test]
    fn sin_clave_de_recuperacion_no_se_lee_ninguna_revocacion() {
        // Un borrador de `arrancar_con_almacen` sustituia la clave ausente por
        // la operativa envuelta en el dominio de recuperacion. Era un agujero:
        // quien tuviera la privada operativa habria podido forjar un certificado
        // que verificase y bajar el centinela con `reiniciar_por`.
        let almacen = AlmacenDePrueba::nuevo("sin-recuperacion");
        let banco = banco(DominioClave::Cliente);

        aprovisionar_clave(
            &almacen.rutas.clave_operativa(),
            banco.clave.verificacion(),
            DominioClave::Cliente,
        )
        .expect("aprovisionar operativa");

        std::fs::write(
            almacen.rutas.revocaciones(),
            ArchivoRevocaciones::nuevo().serializar(),
        )
        .expect("escribir");

        let registro = arranque::cargar_revocaciones(&almacen.rutas, None).expect("cargar");

        assert_eq!(
            registro,
            RegistroRevocaciones::default(),
            "sin clave de recuperacion el registro queda vacio, no se cree nada"
        );

        assert!(
            cargar_clave(
                &almacen.rutas.clave_recuperacion(),
                DominioClave::ClienteRecuperacion
            )
            .expect("no es un fallo de disco")
            .is_none(),
            "y su ausencia no impide arrancar"
        );

        assert!(arrancar_con_almacen(&almacen.rutas).is_ok());
    }

    // -----------------------------------------------------------------------
    // Declaracion firmada de segmentos — RPT-022, PA-45
    // -----------------------------------------------------------------------

    /// Desplazamiento del primer registro de segmento en el fichero.
    fn inicio_de_segmentos(banco: &Banco) -> usize {
        CABECERA + banco.inventario.marcados().len() * ENTRADA
    }

    #[test]
    fn las_naturalezas_de_segmento_coinciden_con_el_manifiesto() {
        let declaradas = valores_bajo(&manifiesto(), "[[naturaleza_segmento]]", "nombre");
        let implementadas: Vec<String> = NaturalezaSegmento::TODAS
            .iter()
            .map(|naturaleza| naturaleza.identificador().to_owned())
            .collect();

        assert_eq!(
            declaradas, implementadas,
            "las naturalezas de segmento divergen del manifiesto.\n  \
             manifiesto: {declaradas:?}\n  \
             codigo    : {implementadas:?}"
        );
    }

    #[test]
    fn no_existe_codigo_para_la_ausencia_de_declaracion() {
        // «No declarado» se representa con la AUSENCIA de registro. Si tuviera
        // codigo propio, el mismo estado tendria dos representaciones y volveria
        // la ambiguedad que el resto del formato cierra.
        assert_eq!(NaturalezaSegmento::TODAS.len(), 2);

        for codigo in 0u8..=255 {
            let naturaleza = NaturalezaSegmento::desde_codigo(codigo);

            if let Some(naturaleza) = naturaleza {
                assert_eq!(naturaleza.codigo(), codigo, "el codigo es reversible");
                assert_ne!(
                    naturaleza.a_declaracion(),
                    DeclaracionSegmento::NoDeclarado,
                    "ninguna naturaleza codificable significa «no declarado»"
                );
            }
        }

        assert_eq!(
            NaturalezaSegmento::desde_codigo(0),
            None,
            "el cero queda reservado: un bloque de ceros no es una tabla valida"
        );
    }

    #[test]
    fn alterar_una_declaracion_de_segmento_invalida_la_firma() {
        // El nucleo de PA-45. Si la tabla viviera fuera del manifiesto firmado,
        // esta edicion de un byte —declarar limpia la VLAN clinica— volveria
        // contenible a todo equipo sin marcado de ese segmento.
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);

        // La VLAN 40 es la clinica y va segunda en orden ascendente.
        let posicion = inicio_de_segmentos(&banco) + SEGMENTO + 2;
        assert_eq!(
            bytes[posicion],
            NaturalezaSegmento::PuedeAlojarCriticos.codigo()
        );
        bytes[posicion] = NaturalezaSegmento::SinDispositivosCriticos.codigo();

        assert!(
            analizar(&bytes).is_ok(),
            "el fichero sigue estructuralmente bien formado"
        );
        assert_eq!(
            InventarioLocal::cargar(&bytes, &banco.clave, banco.centinela, &banco.revocaciones)
                .err(),
            Some(ErrorCarga::Verificacion(
                ErrorInventario::FirmaDeRaizInvalida
            )),
            "pero la firma cubre el resumen de la tabla, asi que deja de verificar"
        );
    }

    #[test]
    fn borrar_el_bloque_de_segmentos_entero_tampoco_pasa_desapercibido() {
        // La proteccion no puede depender de que el bloque exista: hay que
        // distinguir «tabla vacia» de «tabla borrada». Por eso el numero de
        // declaraciones se absorbe ANTES que las declaraciones.
        let banco = banco(DominioClave::Cliente);
        let inicio = inicio_de_segmentos(&banco);
        let cuantos = banco.vlans.declaraciones().len();

        let mut mutilado = bytes_en_disco(&banco);
        mutilado[CABECERA - 2..CABECERA].copy_from_slice(&0u16.to_be_bytes());
        mutilado.drain(inicio..inicio + cuantos * SEGMENTO);

        let fichero = analizar(&mutilado).expect("sigue bien formado: una tabla vacia es legitima");
        assert!(fichero.vlans.declaraciones().is_empty());
        assert_ne!(
            fichero.anclada.vlans, banco.anclada.vlans,
            "el resumen de la tabla vacia no es el de la tabla con contenido"
        );

        assert_eq!(
            InventarioLocal::cargar(
                &mutilado,
                &banco.clave,
                banco.centinela,
                &banco.revocaciones
            )
            .err(),
            Some(ErrorCarga::Verificacion(
                ErrorInventario::FirmaDeRaizInvalida
            ))
        );
    }

    #[test]
    fn una_tabla_vacia_no_comparte_resumen_con_ninguna_otra() {
        let vacia = TablaVlan::vacia();
        let una = TablaVlan::construir(vec![DeclaracionVlan {
            vlan: VLAN_LIMPIA,
            naturaleza: NaturalezaSegmento::SinDispositivosCriticos,
            emitido_en: AHORA,
            vigencia_dias: 365,
        }])
        .expect("una sola");

        assert_ne!(vacia.resumen(), una.resumen());
        assert_ne!(una.resumen(), banco(DominioClave::Cliente).vlans.resumen());
    }

    #[test]
    fn el_orden_de_entrada_no_altera_el_resumen_de_la_tabla() {
        let mut invertidas = segmentos_de_prueba();
        invertidas.reverse();

        assert_eq!(
            TablaVlan::construir(segmentos_de_prueba())
                .expect("sin repetidas")
                .resumen(),
            TablaVlan::construir(invertidas)
                .expect("sin repetidas")
                .resumen()
        );
    }

    #[test]
    fn una_vlan_declarada_dos_veces_se_rechaza() {
        // Elegir la primera o la ultima es arbitrario, y una de las dos
        // elecciones favorece a quien anade un segundo registro «limpio».
        let repetida = vec![
            DeclaracionVlan {
                vlan: VLAN_CLINICA,
                naturaleza: NaturalezaSegmento::PuedeAlojarCriticos,
                emitido_en: AHORA,
                vigencia_dias: 365,
            },
            DeclaracionVlan {
                vlan: VLAN_CLINICA,
                naturaleza: NaturalezaSegmento::SinDispositivosCriticos,
                emitido_en: AHORA,
                vigencia_dias: 365,
            },
        ];

        assert_eq!(
            TablaVlan::construir(repetida).err(),
            Some(ErrorVlan::VlanDuplicada { vlan: VLAN_CLINICA })
        );
    }

    #[test]
    fn la_vlan_cero_y_la_reservada_no_son_declarables() {
        // El VID 0 significa «etiquetado solo por prioridad, sin pertenencia a
        // VLAN», y `eje-captura` lo entrega como Some(0). Declararlo limpio
        // concederia contencion a cualquiera que emita tramas con prioridad.
        for vlan in [0, VLAN_MAXIMA + 1, 0xFFFF] {
            assert_eq!(
                TablaVlan::construir(vec![DeclaracionVlan {
                    vlan,
                    naturaleza: NaturalezaSegmento::SinDispositivosCriticos,
                    emitido_en: AHORA,
                    vigencia_dias: 365,
                }])
                .err(),
                Some(ErrorVlan::VlanFueraDeRango { vlan }),
                "la vlan {vlan} no debe ser declarable"
            );
        }

        assert_eq!(VLAN_MINIMA, 1);
    }

    #[test]
    fn un_codigo_de_naturaleza_desconocido_se_rechaza_al_leer() {
        // Incluido el cero: aceptarlo dejaria que un bloque de relleno se
        // analizase como declaraciones validas.
        let banco = banco(DominioClave::Cliente);

        for codigo in [0u8, 3, 200] {
            let mut bytes = bytes_en_disco(&banco);
            bytes[inicio_de_segmentos(&banco) + 2] = codigo;

            assert_eq!(
                analizar(&bytes).err(),
                Some(ErrorFormato::NaturalezaDesconocida { codigo }),
                "el codigo {codigo} no debe interpretarse"
            );
        }
    }

    #[test]
    fn una_vlan_fuera_de_rango_en_disco_se_rechaza() {
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);
        let inicio = inicio_de_segmentos(&banco);
        bytes[inicio..inicio + 2].copy_from_slice(&0u16.to_be_bytes());

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::VlanFueraDeRango { vlan: 0 })
        );
    }

    #[test]
    fn los_segmentos_desordenados_en_disco_se_rechazan() {
        // Mismo motivo que las entradas: reordenar en silencio haria que dos
        // ficheros distintos produjeran el mismo resumen firmado.
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);
        let inicio = inicio_de_segmentos(&banco);

        let uno: Vec<u8> = bytes[inicio..inicio + SEGMENTO].to_vec();
        let dos: Vec<u8> = bytes[inicio + SEGMENTO..inicio + 2 * SEGMENTO].to_vec();

        bytes[inicio..inicio + SEGMENTO].copy_from_slice(&dos);
        bytes[inicio + SEGMENTO..inicio + 2 * SEGMENTO].copy_from_slice(&uno);

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::SegmentosDesordenados { posicion: 1 })
        );
    }

    #[test]
    fn solo_una_declaracion_firmada_vigente_declara_limpio_un_segmento() {
        // El recorrido que PA-45 abre: de la etiqueta VLAN observada a una
        // declaracion que un humano firmo.
        let almacen = AlmacenDePrueba::nuevo("segmentos-firmados");
        let banco = banco(DominioClave::Cliente);
        let bytes = bytes_en_disco(&banco);
        aceptar_inventario(&almacen.rutas, &bytes, 7).expect("aceptar");

        let (estado, _) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("arranque");

        assert!(matches!(estado, EstadoArranque::Operativo(_)));

        assert_eq!(
            estado.declaracion_para(Some(VLAN_LIMPIA), AHORA),
            DeclaracionSegmento::SinDispositivosCriticos
        );
        assert_eq!(
            estado.declaracion_para(Some(VLAN_CLINICA), AHORA),
            DeclaracionSegmento::PuedeAlojarCriticos
        );
        assert_eq!(
            estado.declaracion_para(Some(999), AHORA),
            DeclaracionSegmento::NoDeclarado,
            "una vlan sin registro no esta declarada"
        );
        assert_eq!(
            estado.declaracion_para(None, AHORA),
            DeclaracionSegmento::NoDeclarado,
            "una trama sin etiqueta no hereda la declaracion de nadie"
        );
    }

    #[test]
    fn una_declaracion_caducada_deja_de_declarar_limpio() {
        // La declaracion peligrosa es «este segmento esta limpio», y es la que
        // envejece mal: nadie reemite el manifiesto el dia que se conecta un
        // carro de telemedicina a la VLAN administrativa.
        let tabla = TablaVlan::construir(segmentos_de_prueba()).expect("sin repetidas");
        let declaracion = tabla
            .declaracion_de(VLAN_LIMPIA)
            .copied()
            .expect("la vlan figura");

        assert!(declaracion.vigente_en(AHORA));
        assert!(!declaracion.vigente_en(AHORA + 366 * 86_400));

        // Y un reloj atrasado tambien caduca, en lugar de extender la vigencia.
        assert!(!declaracion.vigente_en(AHORA - 1));
    }

    #[test]
    fn sin_manifiesto_verificado_ningun_segmento_esta_declarado() {
        // Los cuatro estados no operativos coinciden aqui, y no por comodidad:
        // sin manifiesto nadie ha firmado que ningun segmento este limpio.
        for estado in [
            EstadoArranque::PrimerArranque,
            EstadoArranque::FormatoObsoleto { encontrada: 1 },
            EstadoArranque::Supresion {
                secuencia_conocida: 7,
            },
            EstadoArranque::NoVerifica {
                detalle: "prueba".to_owned(),
            },
        ] {
            assert_eq!(
                estado.declaracion_para(Some(VLAN_LIMPIA), AHORA),
                DeclaracionSegmento::NoDeclarado,
                "sin manifiesto no se puede declarar limpio nada"
            );
            assert!(
                estado
                    .declaracion_para(Some(VLAN_LIMPIA), AHORA)
                    .admite_criticos(),
                "y el lado seguro es admitir criticos"
            );
        }
    }

    #[test]
    fn una_tabla_ajena_no_se_puede_hacer_pasar_por_verificada() {
        // `TablaVlanVerificada` no tiene mas constructor que este. Sin la
        // comprobacion, cualquier modulo futuro podria cargar la tabla de un
        // fichero de configuracion y presentarla como firmada.
        let banco = banco(DominioClave::Cliente);
        let raiz = banco.raiz_verificada().expect("la raiz verifica");

        let ajena = TablaVlan::construir(vec![DeclaracionVlan {
            vlan: VLAN_CLINICA,
            naturaleza: NaturalezaSegmento::SinDispositivosCriticos,
            emitido_en: AHORA,
            vigencia_dias: 365,
        }])
        .expect("una sola");

        assert_eq!(
            TablaVlanVerificada::verificar_e_instanciar(ajena, &raiz).err(),
            Some(ErrorVlan::TablaAjenaAlManifiesto)
        );

        assert!(
            TablaVlanVerificada::verificar_e_instanciar(banco.vlans.clone(), &raiz).is_ok(),
            "la tabla del propio manifiesto si"
        );
    }

    #[test]
    fn solo_la_manipulacion_es_manipulacion() {
        // Barrido de los cinco estados. Si alguien anade uno nuevo y olvida
        // clasificarlo, esta prueba no lo ve — pero el `match` de
        // `es_manipulacion` deja de compilar si el enum crece, que es la mitad
        // que si esta cubierta.
        let manipulaciones = [
            EstadoArranque::Supresion {
                secuencia_conocida: 7,
            },
            EstadoArranque::NoVerifica {
                detalle: "prueba".to_owned(),
            },
        ];
        for estado in manipulaciones {
            assert!(estado.es_manipulacion(), "{}", estado.exige_alerta());
            assert!(estado.exige_alerta());
        }

        let benignos = [
            EstadoArranque::PrimerArranque,
            EstadoArranque::FormatoObsoleto { encontrada: 0 },
        ];
        for estado in benignos {
            assert!(!estado.es_manipulacion());
        }

        assert!(!EstadoArranque::PrimerArranque.exige_alerta());
        assert!(EstadoArranque::FormatoObsoleto { encontrada: 0 }.exige_alerta());
    }

    #[test]
    fn el_ciclo_completo_sobrevive_a_un_reinicio() {
        // PA-35 de extremo a extremo: escribir, «reiniciar» volviendo a arrancar,
        // y comprobar que el marcado sigue ahi y la frescura se conserva.
        let almacen = AlmacenDePrueba::nuevo("reinicio");
        let banco = banco(DominioClave::Cliente);

        let centinelas = aceptar_inventario(&almacen.rutas, &bytes_en_disco(&banco), 7)
            .expect("aceptar el inventario");
        assert_eq!(centinelas.inventario, Centinela::Establecido(7));
        assert_eq!(
            cargar_centinela(&almacen.rutas).expect("releer"),
            centinelas
        );

        let (estado, centinela) = arrancar(
            &almacen.rutas,
            &banco.clave,
            Some(&clave_recuperacion_de_prueba()),
        )
        .expect("rearranque");

        assert!(matches!(estado, EstadoArranque::Operativo(_)));
        assert_eq!(centinela.inventario, Centinela::Establecido(7));

        let marcado = estado
            .marcado(&MAC)
            .expect("el proveedor responde")
            .expect("la mac figura");
        assert_eq!(marcado.clase(), Some(ClaseExcluida::SoporteVital));
    }

    #[test]
    fn el_centinela_se_escribe_antes_que_el_inventario() {
        // Si el proceso muriera entre ambas escrituras, el centinela ya estaria
        // en N. Se comprueba el orden observando el estado tras la primera.
        let almacen = AlmacenDePrueba::nuevo("orden-de-escritura");

        disco::escribir_atomico(
            &almacen.rutas.centinela(),
            &serializar_centinela(
                Centinelas::SIN_ESTABLECER.con_inventario(Centinela::Establecido(9)),
            ),
        )
        .expect("escritura del centinela");

        assert_eq!(
            cargar_centinela(&almacen.rutas).expect("releer").inventario,
            Centinela::Establecido(9),
            "el centinela debe quedar en disco aunque el inventario no haya llegado"
        );
        assert!(
            !almacen.rutas.inventario().exists(),
            "el inventario aun no se escribio"
        );
    }

    #[test]
    fn las_rutas_persistentes_cuelgan_de_un_solo_directorio() {
        let rutas = RutasAlmacen::nuevo(PathBuf::from("/datos/eje"));

        for ruta in [
            rutas.inventario(),
            rutas.revocaciones(),
            rutas.centinela(),
            rutas.evidencia(),
            rutas.ancla_evidencia(),
        ] {
            assert_eq!(ruta.parent(), Some(rutas.directorio()));
        }
    }

    /// El socket es lo unico que **no** cuelga del directorio de datos.
    ///
    /// RPT-067, PA-120. Mientras compartieron sitio, `ReadWritePaths` autorizaba
    /// a la vez escribir evidencia y crear el socket, y medir el aislamiento del
    /// servicio no distinguia una cosa de la otra.
    #[test]
    fn el_socket_no_vive_donde_la_evidencia() {
        let rutas = RutasAlmacen::nuevo(PathBuf::from("/var/lib/eje-latam"));

        assert_ne!(rutas.socket().parent(), Some(rutas.directorio()));
        assert_eq!(
            rutas.socket(),
            PathBuf::from(arranque::DIRECTORIO_SOCKET_POR_OMISION).join("agente.sock")
        );
    }

    // -----------------------------------------------------------------------
    // Configuracion firmada — RPT-074, PA-79
    // -----------------------------------------------------------------------
    //
    // El analizador corre ANTES de que ninguna firma se verifique, sobre un
    // fichero que el modelo de amenazas asume manipulable. Vale aqui todo lo que
    // `formato` exige de si mismo.

    fn valores_de_prueba() -> configuracion::Valores {
        configuracion::Valores {
            secuencia: 7,
            interfaz: "eth0".to_owned(),
            perfil: PerfilSegmento::Ot,
            colector: "siem.hospital:514".to_owned(),
            intervalo_latido_ms: 60_000,
            grupo_ipc: Some(1000),
            nombre: "sensor-planta-3".to_owned(),
            maquina_esperada: "planta-3".to_owned(),
        }
    }

    /// Un sensor que nunca acepto una configuracion.
    ///
    /// RPT-078. La mayoria de estas pruebas examinan el **formato** y la firma, no
    /// la frescura, y sin marca la frescura no rechaza nada. Que haya que
    /// escribirlo en cada llamada es deliberado: `analizar` no deja leer una
    /// configuracion sin decir contra que se fecha, y por eso la comprobacion no
    /// se puede quedar sin cablear.
    const fn sin_marca() -> Centinela {
        Centinela::SinEstablecer
    }

    /// Fichero valido con su firma, y la clave que lo verifica.
    ///
    /// La clave sale **envuelta en su dominio**: el verificador la exige asi para
    /// que una clave de PremosCorp no pueda decidir a que segmento apunta un
    /// sensor (RPT-011 §4).
    fn configuracion_firmada(semilla: u64) -> (Vec<u8>, ClaveInventario, configuracion::Valores) {
        let (firmante, verificadora) = generar_par(&mut GeneradorDeterminista::nuevo(semilla));
        let valores = valores_de_prueba();
        let firma = firmar(
            &firmante,
            &configuracion::mensaje_de_configuracion(&valores),
        );

        (
            configuracion::serializar(&valores, &firma),
            ClaveInventario::nueva(verificadora, DominioClave::Cliente),
            valores,
        )
    }

    /// La clave de PremosCorp no configura sensores del cliente.
    ///
    /// RPT-074 §3. Es la razon de que el verificador reciba la clave **envuelta**
    /// y no desnuda: con la clave suelta, quien llamara elegiria con cual
    /// verificar, y la separacion de dominios dependeria de que nadie confunda
    /// dos variables.
    #[test]
    fn la_clave_de_premoscorp_no_firma_configuraciones() {
        let (firmante, verificadora) = generar_par(&mut GeneradorDeterminista::nuevo(0x7E));
        let valores = valores_de_prueba();
        let firma = firmar(
            &firmante,
            &configuracion::mensaje_de_configuracion(&valores),
        );
        let bytes = configuracion::serializar(&valores, &firma);

        // La firma es valida; lo que no vale es el dominio de la clave.
        let ajena = ClaveInventario::nueva(verificadora, DominioClave::PremosCorp);

        assert!(matches!(
            configuracion::analizar(&bytes, &ajena, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::DominioInesperado {
                encontrado: DominioClave::PremosCorp
            })
        ));
    }

    #[test]
    fn una_configuracion_firmada_va_y_vuelve_entera() {
        let (bytes, clave, esperados) = configuracion_firmada(0xC0_FF);

        let leidos =
            configuracion::analizar(&bytes, &clave, &esperados.maquina_esperada, sin_marca())
                .expect("la configuracion recien firmada tiene que verificar");

        assert_eq!(leidos, esperados);
    }

    /// La defensa contra el traslado lateral.
    ///
    /// Sin ella, root copia la configuracion de un sensor tranquilo sobre uno
    /// ruidoso y **las dos firmas son legitimas**.
    #[test]
    fn una_configuracion_de_otra_maquina_se_rechaza_pese_a_firma_valida() {
        let (bytes, clave, _) = configuracion_firmada(0xA1);

        let error = configuracion::analizar(&bytes, &clave, "planta-9", sin_marca())
            .expect_err("una configuracion de otra maquina no puede valer aqui");

        assert!(
            matches!(
                error,
                configuracion::ErrorConfiguracion::MaquinaAjena { .. }
            ),
            "el motivo tiene que decir que es de otra maquina: {error:?}"
        );
    }

    /// Y la maquina se compara **despues** de la firma, nunca antes.
    ///
    /// Al reves, un fichero inventado decidiria contra que nombre se compara, y
    /// bastaria con poner el hostname correcto para pasar la puerta.
    #[test]
    fn un_fichero_inventado_acusa_a_la_firma_y_no_a_la_maquina() {
        let (bytes, _, _) = configuracion_firmada(0xB2);
        let (_, otra) = generar_par(&mut GeneradorDeterminista::nuevo(0xB3));
        let otra_clave = ClaveInventario::nueva(otra, DominioClave::Cliente);

        let error = configuracion::analizar(&bytes, &otra_clave, "planta-9", sin_marca())
            .expect_err("firmado por otra clave no puede verificar");

        assert!(
            matches!(error, configuracion::ErrorConfiguracion::FirmaInvalida),
            "con la firma rota, la maquina no se ha llegado a mirar: {error:?}"
        );
    }

    #[test]
    fn alterar_un_campo_invalida_la_firma_de_la_configuracion() {
        let (mut bytes, clave, valores) = configuracion_firmada(0xC3);

        // Primer byte del primer campo de texto: cabecera (32) + prefijo (2).
        bytes[34] ^= 0x01;

        let error = configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca())
            .expect_err("un campo alterado no puede verificar");

        assert!(matches!(
            error,
            configuracion::ErrorConfiguracion::FirmaInvalida
        ));
    }

    #[test]
    fn un_magico_ajeno_no_es_una_configuracion() {
        let (mut bytes, clave, valores) = configuracion_firmada(0xD4);
        bytes[0] = b'X';

        assert!(matches!(
            configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::MagicoAusente)
        ));
    }

    /// Una version futura se rechaza en lugar de interpretarse.
    ///
    /// Puede significar campos nuevos, y leerla a medias es inventarse el
    /// contenido de un fichero que decide como se comporta el sensor.
    #[test]
    fn una_version_futura_de_configuracion_se_rechaza() {
        let (mut bytes, clave, valores) = configuracion_firmada(0xE5);
        bytes[8] = 0;
        bytes[9] = 9;

        assert!(matches!(
            configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::VersionDesconocida { encontrada: 9 })
        ));
    }

    /// El cero no es un perfil: un fichero de ceros no produce configuracion.
    #[test]
    fn un_perfil_desconocido_se_rechaza_en_la_puerta() {
        let (bytes, clave, valores) = configuracion_firmada(0xF6);

        for codigo in [0u8, 9, 255] {
            let mut tocado = bytes.clone();
            tocado[18] = codigo;

            assert!(
                matches!(
                    configuracion::analizar(
                        &tocado,
                        &clave,
                        &valores.maquina_esperada,
                        sin_marca()
                    ),
                    Err(configuracion::ErrorConfiguracion::PerfilDesconocido { .. })
                ),
                "el codigo {codigo} no es un perfil y no debe leerse como uno"
            );
        }
    }

    /// Un intervalo de cero convertiria cada vuelta en un latido.
    #[test]
    fn un_intervalo_de_latido_nulo_se_rechaza() {
        let (mut bytes, clave, valores) = configuracion_firmada(0x17);
        bytes[19..27].fill(0);

        assert!(matches!(
            configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::IntervaloImposible { encontrado: 0 })
        ));
    }

    /// Un prefijo absurdo no llega a indexar ni a reservar.
    ///
    /// Es la leccion de `eje-ipc`: el techo se comprueba antes de cortar, no
    /// despues de intentarlo.
    #[test]
    fn un_campo_de_longitud_absurda_no_desborda() {
        let (mut bytes, clave, valores) = configuracion_firmada(0x28);
        bytes[32] = 0xFF;
        bytes[33] = 0xFF;

        assert!(matches!(
            configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::CampoImposible { indice: 0 })
        ));
    }

    #[test]
    fn un_campo_que_no_es_texto_se_rechaza() {
        let (mut bytes, clave, valores) = configuracion_firmada(0x39);
        // Secuencia UTF-8 imposible dentro del primer campo.
        bytes[34] = 0xFF;

        assert!(matches!(
            configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::CampoNoEsTexto { indice: 0 })
        ));
    }

    /// Una cola sobrante admitiria dos lecturas del mismo fichero.
    #[test]
    fn una_cola_sobrante_en_la_configuracion_se_rechaza() {
        let (mut bytes, clave, valores) = configuracion_firmada(0x4A);
        bytes.push(0);

        assert!(matches!(
            configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::FirmaMalformada)
        ));
    }

    /// Un fichero vacio o minusculo no desborda.
    #[test]
    fn una_configuracion_truncada_no_desborda() {
        let (bytes, clave, valores) = configuracion_firmada(0x5B);

        for corte in [0usize, 1, 8, 17, 31, 33, 40] {
            let recortada = &bytes[..corte.min(bytes.len())];
            assert!(
                configuracion::analizar(recortada, &clave, &valores.maquina_esperada, sin_marca())
                    .is_err(),
                "un fichero de {corte} bytes no puede producir configuracion"
            );
        }
    }

    /// «Sin grupo» y «grupo 0» son cosas distintas y el mensaje lo refleja.
    ///
    /// El cero es `root`. Sin la bandera de presencia, un socket sin grupo y uno
    /// del grupo de root producirian el mismo mensaje firmado, y una firma
    /// emitida para lo uno valdria para lo otro.
    #[test]
    fn sin_grupo_y_grupo_cero_no_firman_lo_mismo() {
        let sin_grupo = configuracion::Valores {
            grupo_ipc: None,
            ..valores_de_prueba()
        };
        let grupo_root = configuracion::Valores {
            grupo_ipc: Some(0),
            ..valores_de_prueba()
        };

        assert_ne!(
            configuracion::mensaje_de_configuracion(&sin_grupo),
            configuracion::mensaje_de_configuracion(&grupo_root)
        );
    }

    /// Una configuracion anterior NO entra, aunque su firma sea impecable.
    ///
    /// RPT-078, PA-79 paso 5. Es el ataque entero: la firma de la configuracion
    /// de la semana pasada —la del intervalo de latido largo, la que apuntaba a
    /// otro colector— es perfectamente valida. Lo que no vale es reponerla sobre
    /// un sensor que ya vio una posterior.
    ///
    /// La misma se admite a proposito: `aceptar_configuracion` avanza la marca
    /// **antes** de obedecer, asi que un corte entre las dos cosas deja la marca
    /// en N con la N sin aplicar. Con una comparacion estricta ese sensor no
    /// volveria a arrancar nunca.
    #[test]
    fn una_configuracion_revertida_se_rechaza_pese_a_firma_valida() {
        let (bytes, clave, valores) = configuracion_firmada(0x5E);

        assert!(
            configuracion::analizar(
                &bytes,
                &clave,
                &valores.maquina_esperada,
                Centinela::Establecido(valores.secuencia)
            )
            .is_ok(),
            "reponer la MISMA secuencia tiene que seguir arrancando"
        );

        assert!(
            matches!(
                configuracion::analizar(
                    &bytes,
                    &clave,
                    &valores.maquina_esperada,
                    Centinela::Establecido(valores.secuencia + 1)
                ),
                Err(configuracion::ErrorConfiguracion::SecuenciaRevertida { .. })
            ),
            "una configuracion anterior a la ultima aceptada entro con firma valida"
        );
    }

    /// Y una secuencia en el techo no congela el sensor para siempre.
    ///
    /// RPT-078, y es PA-33 aplicado a la configuracion. Sin techo, **un solo
    /// mensaje** con `u64::MAX` deja la marca donde ninguna configuracion legitima
    /// puede alcanzarla, y revocar la clave no lo arregla porque la marca sigue
    /// arriba. Se rechaza como malformacion, antes de mirar nada mas.
    #[test]
    fn una_secuencia_de_configuracion_en_el_techo_se_rechaza() {
        let (firmante, verificadora) = generar_par(&mut GeneradorDeterminista::nuevo(0x33));
        let valores = configuracion::Valores {
            secuencia: u64::MAX,
            ..valores_de_prueba()
        };
        let firma = firmar(
            &firmante,
            &configuracion::mensaje_de_configuracion(&valores),
        );
        let bytes = configuracion::serializar(&valores, &firma);
        let clave = ClaveInventario::nueva(verificadora, DominioClave::Cliente);

        assert!(matches!(
            configuracion::analizar(&bytes, &clave, &valores.maquina_esperada, sin_marca()),
            Err(configuracion::ErrorConfiguracion::SecuenciaFueraDeRango { .. })
        ));
    }

    /// Aceptar un inventario **no** borra la marca de configuracion.
    ///
    /// RPT-078. Es el fallo que el contenedor comun existe para impedir, y no se
    /// ve leyendo ninguno de los dos ficheros por separado: `aceptar_inventario`
    /// componia el centinela entero desde la secuencia del inventario, asi que un
    /// segundo escritor ingenuo habria dejado la marca de configuracion a cero en
    /// silencio — y a partir de ahi cualquier configuracion antigua y bien firmada
    /// habria entrado.
    ///
    /// Se prueba en disco y en los dos ordenes, porque uno solo dejaria pasar la
    /// mitad del error.
    #[test]
    fn las_dos_marcas_sobreviven_a_que_la_otra_avance() {
        let almacen = AlmacenDePrueba::nuevo("marcas-cruzadas");
        let banco = banco(DominioClave::Cliente);

        aceptar_configuracion(&almacen.rutas, 4).expect("anotar configuracion");
        let tras_inventario = aceptar_inventario(&almacen.rutas, &bytes_en_disco(&banco), 7)
            .expect("aceptar inventario");

        assert_eq!(
            tras_inventario.configuracion,
            Centinela::Establecido(4),
            "aceptar un inventario borro la marca de configuracion"
        );

        let tras_configuracion =
            aceptar_configuracion(&almacen.rutas, 5).expect("anotar configuracion posterior");

        assert_eq!(
            tras_configuracion.inventario,
            Centinela::Establecido(7),
            "anotar una configuracion borro la marca del inventario"
        );
        assert_eq!(
            cargar_centinela(&almacen.rutas).expect("releer"),
            tras_configuracion,
            "lo que quedo en disco no es lo que la funcion dijo haber dejado"
        );
    }

    /// Y dos campos de texto no se pueden intercambiar en silencio.
    ///
    /// Sin prefijos de longitud, `interfaz="eth" nombre="0"` e
    /// `interfaz="eth0" nombre=""` darian el mismo mensaje canonico.
    #[test]
    fn los_prefijos_impiden_confundir_dos_campos_de_configuracion() {
        let uno = configuracion::Valores {
            interfaz: "eth".to_owned(),
            nombre: "0".to_owned(),
            ..valores_de_prueba()
        };
        let otro = configuracion::Valores {
            interfaz: "eth0".to_owned(),
            nombre: String::new(),
            ..valores_de_prueba()
        };

        assert_ne!(
            configuracion::mensaje_de_configuracion(&uno),
            configuracion::mensaje_de_configuracion(&otro)
        );
    }

    /// Arnes de mutacion determinista sobre un fichero valido.
    ///
    /// **Esto no es fuzzing** (RPT-014 §4): un mutador ciego con semilla fija
    /// explora un espacio minusculo y no crece. Vale como red de regresion y como
    /// guardia contra panicos evidentes; la afirmacion fuerte la sostendra el
    /// objetivo de `cargo-fuzz`.
    ///
    /// Lo unico que se exige es que **ninguna entrada produzca panico** y que
    /// ninguna mutacion produzca una configuracion verificada distinta de la
    /// original: una firma no puede avalar bytes que nadie firmo.
    #[test]
    fn el_analizador_de_configuracion_resiste_mutaciones() {
        let (bytes, clave, valores) = configuracion_firmada(0x6C);
        let mut generador = GeneradorDeterminista::nuevo(0x6D);

        for _ in 0..2_000 {
            let mut tocado = bytes.clone();
            let posicion = (generador.siguiente() as usize) % tocado.len();
            let valor = (generador.siguiente() % 256) as u8;
            tocado[posicion] = valor;

            match configuracion::analizar(&tocado, &clave, &valores.maquina_esperada, sin_marca()) {
                Err(_) => {}
                Ok(leidos) => assert_eq!(
                    leidos, valores,
                    "una mutacion produjo una configuracion verificada DISTINTA"
                ),
            }
        }
    }

    /// Y el nombre del fichero no se puede mover, solo el directorio.
    ///
    /// Si la ruta completa fuera configurable, nada impediria devolverla al
    /// directorio de evidencia y deshacer la separacion en silencio.
    #[test]
    fn el_directorio_del_socket_se_puede_mover_y_el_nombre_no() {
        let rutas = RutasAlmacen::con_directorio_socket(
            PathBuf::from("/var/lib/eje-latam"),
            PathBuf::from("/tmp/eje"),
        );

        assert_eq!(rutas.socket(), PathBuf::from("/tmp/eje/agente.sock"));
        assert_eq!(
            rutas.directorio(),
            PathBuf::from("/var/lib/eje-latam").as_path()
        );
        assert_eq!(
            rutas.directorio_socket(),
            PathBuf::from("/tmp/eje").as_path()
        );
    }

    #[test]
    fn el_centinela_no_baja_sin_certificado() {
        // `avanzar` sigue siendo monotono; solo `reiniciar_por` baja, y exige un
        // certificado verificado que no se puede fabricar.
        assert_eq!(
            Centinela::Establecido(u64::MAX).avanzar(1),
            Centinela::Establecido(u64::MAX)
        );
    }

    // -----------------------------------------------------------------------
    // Arnes de mutacion determinista — RPT-014, PA-29
    // -----------------------------------------------------------------------
    //
    // ESTO NO ES FUZZING. Un mutador ciego con semilla fija explora un espacio
    // minusculo comparado con la mutacion guiada por cobertura, y sobre todo NO
    // CRECE: repite las mismas rutas en cada ejecucion. Vale como red de
    // regresion y como guardia contra panicos evidentes. La afirmacion de que el
    // analizador resiste entrada hostil la sostiene el objetivo de `cargo-fuzz`
    // bajo nightly, no esto.

    /// Comprueba las dos invariantes sobre una entrada arbitraria.
    ///
    /// 1. `analizar` no entra en panico. Se cumple por el mero hecho de volver:
    ///    un panico aborta la prueba.
    /// 2. Si acepta, la codificacion es **canonica**: reserializar devuelve los
    ///    mismos bytes. Sin esta segunda, un analizador que normalizase en
    ///    silencio pasaria por bueno, y con el la codificacion en disco dejaria
    ///    de ser unica.
    fn comprobar_invariantes_del_analizador(caso: &[u8]) {
        if let Ok(fichero) = analizar(caso) {
            let reserializado = serializar(
                &fichero.inventario,
                &fichero.vlans,
                fichero.anclada.secuencia,
                &fichero.firma,
            );

            // La comparacion se acota a la parte estructural, excluyendo la
            // firma. Motivo: nada garantiza que `encode(decode(x))` devuelva los
            // mismos bytes para una firma mutada que aun decodifique, y una
            // normalizacion del blob criptografico haria fallar la prueba por un
            // motivo ajeno al analizador. La canonicidad que importa aqui es la
            // del inventario: orden, longitudes y campos.
            // Resta comprobada: hoy `analizar` no puede aceptar nada mas corto
            // que la firma, pero apoyarse en esa garantia convertiria un cambio
            // futuro del analizador en un desbordamiento con forma de hallazgo.
            let longitud_firma = motor_pqc::firma_hibrida::FirmaHibrida::longitud_serializada();
            let Some(hasta) = caso.len().checked_sub(longitud_firma) else {
                return;
            };

            assert_eq!(
                reserializado.len(),
                caso.len(),
                "reserializar cambio la longitud de un fichero aceptado"
            );
            assert_eq!(
                &reserializado[..hasta],
                &caso[..hasta],
                "el analizador acepto una codificacion no canonica de {} bytes",
                caso.len()
            );
        }
    }

    /// Aplica una mutacion al caso, elegida de forma determinista.
    fn mutar(caso: &mut Vec<u8>, generador: &mut GeneradorDeterminista) {
        let operacion = generador.siguiente() % 6;

        match operacion {
            0..=2 if !caso.is_empty() => {
                let indice = (generador.siguiente() as usize) % caso.len();
                let byte = match operacion {
                    0 => caso[indice] ^ (1u8 << (generador.siguiente() % 8)),
                    1 => 0x00,
                    _ => 0xFF,
                };
                caso[indice] = byte;
            }
            3 if !caso.is_empty() => {
                // Truncado: el defecto mas comun en un corte de energia.
                let longitud = (generador.siguiente() as usize) % caso.len();
                caso.truncate(longitud);
            }
            4 => {
                // Cola sobrante.
                let cuantos = 1 + (generador.siguiente() % 64) as usize;
                for _ in 0..cuantos {
                    caso.push((generador.siguiente() >> 24) as u8);
                }
            }
            _ => {
                // Los dos contadores de la cabecera, que son los que gobiernan
                // la reserva de memoria. El de segmentos entra aqui desde la
                // version 2: dejarlo fuera habria mantenido el arnes en verde
                // sobre el campo mas nuevo y menos ejercitado.
                if caso.len() >= CABECERA {
                    let valor = (generador.siguiente() as u32).to_be_bytes();
                    caso[18..22].copy_from_slice(&valor);
                    let segmentos = (generador.siguiente() as u16).to_be_bytes();
                    caso[22..24].copy_from_slice(&segmentos);
                }
            }
        }
    }

    #[test]
    fn el_analizador_resiste_mutaciones_sobre_un_fichero_valido() {
        let semilla = bytes_en_disco(&banco(DominioClave::Cliente));
        let mut generador = GeneradorDeterminista::nuevo(0x45_4A_45_2D_49_4E_56);

        // La semilla intacta debe seguir siendo canonica: si esta invariante no
        // se cumpliera de partida, el resto del arnes no probaria nada.
        comprobar_invariantes_del_analizador(&semilla);

        for _ in 0..20_000 {
            let mut caso = semilla.clone();
            let cuantas = 1 + (generador.siguiente() % 4);
            for _ in 0..cuantas {
                mutar(&mut caso, &mut generador);
            }
            comprobar_invariantes_del_analizador(&caso);
        }
    }

    #[test]
    fn el_analizador_de_revocaciones_resiste_mutaciones() {
        // El fichero de revocaciones es la **segunda** entrada no autenticada del
        // producto, y llegó cinco reportes despues que la primera. Sin este
        // arnes, RPT-014 protegeria un analizador y dejaria el otro a la
        // intemperie.
        let banco = banco(DominioClave::Cliente);
        let (firmante, clave) = clave_de_recuperacion(DominioClave::ClienteRecuperacion);

        let mut archivo = ArchivoRevocaciones::nuevo();
        archivo.anotar(anotacion_para(&banco, 7, &firmante));
        let semilla = archivo.serializar();

        let comprobar = |caso: &[u8]| {
            if let Ok(leido) = ArchivoRevocaciones::analizar(caso, &clave) {
                // Aqui la comparacion SI incluye la firma: un `Ok` significa que
                // verifico, y una firma que verifica esta bien formada, asi que
                // reencodearla no puede normalizarla. Es la diferencia con el
                // arnes del inventario, donde `analizar` no verifica nada.
                assert_eq!(
                    leido.serializar().as_slice(),
                    caso,
                    "el analizador acepto una codificacion no canonica de {} bytes",
                    caso.len()
                );
            }
        };

        comprobar(&semilla);

        // Menos casos que en el arnes del inventario, y a proposito.
        //
        // Aqui cada caso estructuralmente valido dispara una verificacion de
        // firma hibrida completa —ML-DSA-65 mas Ed25519—, que en modo depuracion
        // cuesta milisegundos, no microsegundos. Con 10 000 casos este arnes solo
        // llevaba `guardian-cc` de 3 s a 32 s.
        //
        // Los 8 000 casos que se retiran compraban muy poco: el mutador es ciego
        // y con semilla fija, asi que no crece ni explora rutas nuevas por
        // repetir. La afirmacion de resistencia descansa en `cargo-fuzz`
        // (RPT-014 §4), no en el numero de vueltas de aqui.
        let mut generador = GeneradorDeterminista::nuevo(0x52_45_56_4D_55_54);
        for _ in 0..2_000 {
            let mut caso = semilla.clone();
            let cuantas = 1 + (generador.siguiente() % 4);
            for _ in 0..cuantas {
                mutar(&mut caso, &mut generador);
            }
            comprobar(&caso);
        }

        // Y sin semilla, que es donde vive el desbordamiento de indice. Estos son
        // baratos: se rechazan por magico o longitud antes de tocar criptografia.
        for _ in 0..3_000 {
            let longitud = (generador.siguiente() % 64) as usize;
            let caso: Vec<u8> = (0..longitud)
                .map(|_| (generador.siguiente() >> 24) as u8)
                .collect();
            comprobar(&caso);
        }
    }

    #[test]
    fn el_analizador_resiste_bytes_arbitrarios() {
        // Sin semilla valida: casi todo se rechaza por magico o por longitud,
        // pero es donde viven los desbordamientos de indice si el analizador
        // confiara en que hay bytes.
        let mut generador = GeneradorDeterminista::nuevo(0x42_41_53_55_52_41);

        for _ in 0..5_000 {
            let longitud = (generador.siguiente() % 96) as usize;
            let caso: Vec<u8> = (0..longitud)
                .map(|_| (generador.siguiente() >> 24) as u8)
                .collect();
            comprobar_invariantes_del_analizador(&caso);
        }

        // Y con cabecera valida seguida de basura, que llega mas adentro.
        for _ in 0..5_000 {
            let mut caso = Vec::from(*MAGICO);
            caso.extend_from_slice(&1u16.to_be_bytes());
            let longitud = (generador.siguiente() % 96) as usize;
            for _ in 0..longitud {
                caso.push((generador.siguiente() >> 24) as u8);
            }
            comprobar_invariantes_del_analizador(&caso);
        }
    }

    #[test]
    fn un_fichero_con_las_entradas_desordenadas_se_rechaza() {
        // El orden canonico se comprueba al LEER, no solo al construir. Si el
        // analizador reordenase en silencio, dos ficheros distintos darian el
        // mismo inventario y la codificacion dejaria de ser unica.
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);

        // Intercambia las entradas 0 y 1, que estan en orden ascendente.
        let (uno, dos) = (CABECERA, CABECERA + ENTRADA);
        for desplazamiento in 0..19 {
            bytes.swap(uno + desplazamiento, dos + desplazamiento);
        }

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::EntradasDesordenadas { posicion: 1 })
        );
    }

    #[test]
    fn un_fichero_con_la_misma_direccion_dos_veces_se_rechaza_al_leer() {
        // La comprobacion es estrictamente ascendente, asi que la repeticion
        // cae por el mismo camino que el desorden.
        let banco = banco(DominioClave::Cliente);
        let mut bytes = bytes_en_disco(&banco);

        let (uno, dos) = (CABECERA, CABECERA + ENTRADA);
        for desplazamiento in 0..6 {
            bytes[dos + desplazamiento] = bytes[uno + desplazamiento];
        }

        assert_eq!(
            analizar(&bytes).err(),
            Some(ErrorFormato::EntradasDesordenadas { posicion: 1 })
        );
    }

    #[test]
    fn un_dispositivo_declarado_dos_veces_se_rechaza() {
        // Sin este control, un lector indulgente elegiria entre dos entradas
        // contradictorias, y una de las dos elecciones favorece al atacante que
        // anade una segunda entrada «no critico».
        let mut marcados = marcados_de_prueba();
        marcados.push(MarcadoBruto {
            mac: MAC,
            clase: None,
            emitido_en: AHORA,
            vigencia_dias: 365,
        });

        assert_eq!(
            Inventario::construir(marcados),
            Err(ErrorInventario::DispositivoDuplicado { mac: MAC })
        );
    }

    #[test]
    fn la_clave_de_premoscorp_no_firma_inventarios() {
        // Frontera de custodia. PremosCorp firma binarios; el administrador del
        // cliente firma que equipos son criticos. Reutilizar la infraestructura
        // de PA-14 por comodidad permitiria al proveedor declarar que equipos
        // del cliente son incontenibles.
        let banco = banco(DominioClave::PremosCorp);

        assert_eq!(
            banco.verificar(0),
            Err(ErrorInventario::DominioDeClaveIncorrecto {
                encontrado: DominioClave::PremosCorp,
                esperado: DominioClave::Cliente,
            })
        );
    }

    #[test]
    fn el_resumen_del_marcado_separa_su_dominio() {
        // Dos marcados que solo difieren en la clase deben producir hojas
        // distintas. La clase viaja como escalar cerrado, no como cadena.
        let base = MarcadoBruto {
            mac: MAC,
            clase: None,
            emitido_en: AHORA,
            vigencia_dias: 365,
        };
        let mut critico = base;
        critico.clase = Some(ClaseExcluida::SoporteVital);

        assert_ne!(base.resumen(), critico.resumen());

        // Y el mensaje de raiz no coincide con el resumen crudo de la raiz: si
        // coincidieran, una firma sobre 32 bytes cualesquiera valdria como firma
        // de raiz.
        let raiz_falsa = Resumen::desde_bytes([7u8; 32]);
        assert_ne!(
            mensaje_de_raiz(&RaizAnclada {
                raiz: raiz_falsa,
                vlans: TablaVlan::vacia().resumen(),
                secuencia: 1
            }),
            raiz_falsa.bytes().to_vec()
        );
    }

    // --- Vigencia y reloj ---

    #[test]
    fn el_marcado_caduca_al_pasar_su_vigencia() {
        let banco = banco(DominioClave::Cliente);
        let marcado = banco.verificar(0).expect("la cadena esta completa");

        assert!(marcado.vigente_en(AHORA));
        assert!(marcado.vigente_en(AHORA + 365 * 86_400));
        assert!(!marcado.vigente_en(AHORA + 365 * 86_400 + 1));
    }

    #[test]
    fn un_reloj_atrasado_caduca_el_marcado_en_lugar_de_extenderlo() {
        // Ante desviacion de reloj se falla hacia «caducado», que degrada a
        // ambiguo y escala a un humano. Lo contrario permitiria contener un
        // equipo critico con un reloj mal puesto.
        let banco = banco(DominioClave::Cliente);
        let marcado = banco.verificar(0).expect("la cadena esta completa");

        assert!(!marcado.vigente_en(AHORA - 1));
    }

    // --- La prohibicion no puede ser silenciosa ---

    #[test]
    fn toda_prohibicion_exige_alerta_maxima() {
        for clase in ClaseExcluida::TODAS {
            let veredicto = evaluar(
                Clasificacion::Clasificado {
                    clase: Some(clase),
                    fuente: FuenteEvidencia::MarcadoAdministrativo,
                },
                PerfilSegmento::Corporativo,
            );

            assert!(veredicto.exige_alerta());
            assert!(
                veredicto.es_amenaza_incontenible(),
                "una amenaza sobre un equipo que no podemos contener es lo mas \
                 urgente que este producto puede comunicar; bloquear en silencio \
                 convierte la lista de exclusion en una via de evasion comoda"
            );
        }
    }

    #[test]
    fn solo_la_ejecucion_no_alerta() {
        assert!(!evaluar(contenible(), PerfilSegmento::Corporativo).exige_alerta());
        assert!(evaluar(contenible(), PerfilSegmento::Ot).exige_alerta());
        assert!(
            !evaluar(contenible(), PerfilSegmento::Ot).es_amenaza_incontenible(),
            "requerir aprobacion no es una amenaza incontenible; confundirlas \
             ahogaria la senal urgente entre las ordinarias"
        );
    }
}
