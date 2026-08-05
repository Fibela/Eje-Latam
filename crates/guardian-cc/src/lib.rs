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

/// Veredicto previo a emitir una contencion. RPT-008 §5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Veredicto {
    /// Puede ejecutarse sin intervencion.
    Ejecutar,
    /// Debe presentarse a un operador antes de escribir nada.
    ///
    /// El ensayo recorre precondiciones y resolucion de objetivo, y se detiene
    /// justo antes de la escritura.
    RequiereAprobacion,
    /// No puede ejecutarse por ninguna via.
    Prohibida {
        /// Clase que motiva la prohibicion.
        clase: ClaseExcluida,
    },
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

/// Decide si una orden puede ejecutarse, y como. RPT-008 §5.
///
/// El orden de evaluacion importa: la exclusion permanente se comprueba
/// **antes** que el perfil, porque ninguna aprobacion humana la levanta.
#[must_use]
pub fn evaluar(clase: Option<ClaseExcluida>, perfil: PerfilSegmento) -> Veredicto {
    if let Some(clase) = clase {
        return Veredicto::Prohibida { clase };
    }

    if perfil.permite_respuesta_automatica() {
        Veredicto::Ejecutar
    } else {
        Veredicto::RequiereAprobacion
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
            assert_eq!(
                evaluar(Some(clase), PerfilSegmento::Corporativo),
                Veredicto::Prohibida { clase }
            );
            assert_eq!(
                evaluar(Some(clase), PerfilSegmento::Ot),
                Veredicto::Prohibida { clase }
            );
        }
    }

    #[test]
    fn el_perfil_ot_nunca_ejecuta_sin_aprobacion() {
        assert_eq!(
            evaluar(None, PerfilSegmento::Ot),
            Veredicto::RequiereAprobacion
        );
    }

    #[test]
    fn el_perfil_corporativo_ejecuta_si_no_hay_exclusion() {
        assert_eq!(
            evaluar(None, PerfilSegmento::Corporativo),
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
}
