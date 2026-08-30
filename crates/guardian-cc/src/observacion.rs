//! Almacen de observacion partido.
//!
//! RPT-018 §6, PA-38.
//!
//! # El blanqueo por expulsion
//!
//! Una tabla por direccion sin limite es agotamiento de memoria a peticion:
//! basta emitir tramas con direcciones inventadas. Luego hace falta expulsion.
//!
//! Pero `visto_en_segmento_critico` —la ambiguedad pegajosa de RPT-010 §5— vive
//! en ese mismo estado por dispositivo. **Expulsar una entrada olvida que el
//! equipo paso por la VLAN clinica**, y el carro de telemedicina vuelve a ser
//! contenible por el camino que RPT-010 cerro. Es un blanqueo: llenar la tabla
//! hasta que el dispositivo interesante sea expulsado.
//!
//! Por eso el almacen esta partido en dos mitades con politicas opuestas:
//!
//! | | Volatil | Pegajoso |
//! |---|---|---|
//! | Que guarda | protocolos vistos, segmento actual | haber estado en segmento critico |
//! | Crece | rapido | despacio |
//! | Expulsa | si, por antiguedad | **nunca** |
//!
//! # Que pasa si la mitad pegajosa se llena
//!
//! Es la pregunta que el diseno de RPT-018 no contestaba. Expulsar seria el
//! ataque; rechazar entradas nuevas dejaria de proteger a los dispositivos que
//! lleguen despues.
//!
//! La salida es que la saturacion **degrade hacia la seguridad y no hacia el
//! olvido**: si el almacen no puede garantizar la pegajosidad, deja de poder
//! afirmar que un dispositivo sin marcar es contenible, y
//! [`ProveedorSegmento`] pasa a devolver error para **todos**. La clasificacion
//! resuelve en ambiguedad y la contencion automatica se detiene.
//!
//! Llenar la mitad pegajosa deja de blanquear un dispositivo y pasa a bloquear
//! la contencion entera. El atacante no gana un permiso: gana una denegacion,
//! que es la direccion segura.

use std::collections::HashMap;

use crate::ClaseExcluida;
use crate::clasificacion::DeclaracionSegmento;
use crate::proveedores::{
    DireccionEnlace, ErrorProveedor, HistorialSegmento, Indicio, ProveedorHuella, ProveedorSegmento,
};

/// Capacidad de la mitad volatil, en dispositivos.
pub const CAPACIDAD_VOLATIL: usize = 8_192;

/// Fraccion de la mitad volatil que se expulsa al llenarse.
///
/// Se expulsa por lotes en lugar de uno a uno: barrer la tabla en cada insercion
/// convertiria una lluvia de direcciones inventadas en trabajo cuadratico, que
/// es la denegacion de servicio que la expulsion pretendia evitar.
const EXPULSION_POR_LOTE: usize = CAPACIDAD_VOLATIL / 4;

/// Capacidad de la mitad pegajosa, en dispositivos.
///
/// Muy superior a la volatil porque solo anota dispositivos que estuvieron en un
/// segmento critico. Alcanzarla no expulsa nada: ver el encabezado del modulo.
pub const CAPACIDAD_PEGAJOSA: usize = 65_536;

/// Protocolo observado en una trama.
///
/// # Sobre el conjunto
///
/// Este es el conjunto inicial y **no es la taxonomia definitiva**: que
/// protocolos delatan un equipo clinico o industrial es una cuestion de dominio
/// que merece su propio trabajo (RPT-018 §8.1). Los tres primeros vienen de los
/// entornos que RPT-008 nombra.
///
/// `Bacnet` figura a proposito **sin** implicar criticidad: automatiza edificios,
/// y aunque su fallo importe no es soporte vital. Sirve para demostrar que el
/// almacen distingue «observado» de «indicativo».
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Protocolo {
    /// Automatizacion industrial.
    Modbus,
    /// Telecontrol industrial.
    Dnp3,
    /// Mensajeria clinica.
    Hl7,
    /// Automatizacion de edificios.
    Bacnet,
}

impl Protocolo {
    /// Clase que este protocolo **sugiere**, si sugiere alguna.
    ///
    /// Nunca descarta: una fuente inferida no puede afirmar que un equipo no es
    /// critico (RPT-009 §3).
    #[must_use]
    pub const fn sugiere(self) -> Option<ClaseExcluida> {
        match self {
            Self::Modbus | Self::Dnp3 => Some(ClaseExcluida::SeguridadFuncional),
            Self::Hl7 => Some(ClaseExcluida::SoporteVital),
            Self::Bacnet => None,
        }
    }
}

/// Lo observado de un dispositivo, sujeto a expulsion.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EntradaVolatil {
    protocolos: Vec<Protocolo>,
    segmento: DeclaracionSegmento,
    visto_en: u64,
}

/// Lo que el almacen sabe de un dispositivo, para quien tenga que enumerarlos.
///
/// RPT-087, PA-138a.
///
/// # Que NO lleva, que es la mitad del diseno
///
/// No lleva `clase` ni `postura`, los dos campos que `NodoInventario` pide en el
/// contrato. No es un olvido:
///
/// - **La clase puede venir de dos sitios que no valen lo mismo.** Del marcado
///   firmado, o de [`Protocolo::sugiere`], que es una *inferencia* —tanto
///   que existe `un_marcado_no_critico_contradicho_por_la_huella_es_ambiguo`—.
///   Aqui se entregan los protocolos observados en crudo y **quien componga la
///   respuesta decide**, con el marcado delante. Colapsar las dos procedencias
///   en un enumerado plano seria presentar una sospecha como una declaracion.
/// - **La postura no tiene hoy valor para «no se sabe»** (PA-139). Un equipo
///   visto en el cable sin marcado firmado no es conforme, ni anomalo, ni
///   contenido. Inventar uno de los tres es exactamente lo que este punto se
///   abrio para evitar.
///
/// # Y `pegajoso` no significa «contenido»
///
/// Significa que se vio en un segmento que admite criticos y que su marca no se
/// pierde por presion de tabla. **El agente no contiene nada** (RPT-020), asi que
/// leer contencion de aqui seria inventar dato en el punto exacto que PA-138 se
/// abrio para proteger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VistaNodo {
    /// Direccion de capa de enlace. Es la clave del almacen.
    pub direccion: DireccionEnlace,
    /// Protocolos industriales observados, en el orden en que se anotaron.
    pub protocolos: Vec<Protocolo>,
    /// Lo que el administrador declaro del segmento donde se le vio.
    pub segmento: DeclaracionSegmento,
    /// Valor del reloj del almacen en la ultima observacion.
    pub visto_en: u64,
    /// Si su marca resiste la expulsion por presion de tabla.
    pub pegajoso: bool,
}

impl AlmacenObservacion {
    /// Enumera lo observado, en orden estable.
    ///
    /// # Por que el volatil y no el pegajoso
    ///
    /// El pegajoso solo guarda direcciones: de un equipo que solo estuviera ahi
    /// no se podria decir ni que protocolos hablo ni en que segmento se le vio.
    /// El volatil es la unica coleccion que guarda **que** se observo, asi que es
    /// la unica que puede sostener un inventario sin rellenar huecos.
    ///
    /// Un pegajoso ya expulsado del volatil no aparece, y eso es correcto: su
    /// marca sirve para no declararlo contenible a la ligera, no para afirmar que
    /// sigue en la red. Afirmarlo seria inventar presencia.
    ///
    /// # Por que ordenado por direccion
    ///
    /// `HashMap` no promete orden, y un inventario que se reordena solo entre dos
    /// consultas hace parpadear la pantalla del operador y arruina cualquier
    /// comparacion entre vueltas. El orden sale de la clave, que es estable.
    #[must_use]
    pub fn inventario(&self) -> Vec<VistaNodo> {
        let mut nodos: Vec<VistaNodo> = self
            .volatil
            .iter()
            .map(|(direccion, entrada)| VistaNodo {
                direccion: *direccion,
                protocolos: entrada.protocolos.clone(),
                segmento: entrada.segmento,
                visto_en: entrada.visto_en,
                pegajoso: self.pegajoso.contains(direccion),
            })
            .collect();

        nodos.sort_by_key(|nodo| nodo.direccion);
        nodos
    }

    /// Direcciones con marca pegajosa que **ya no estan** en el volatil.
    ///
    /// Es el tercer estado del inventario, y no cabe en [`Self::inventario`]: de
    /// estas se sabe que estuvieron en un segmento critico y **no** se sabe si
    /// siguen. Meterlas en la lista las afirmaria presentes; omitirlas del todo
    /// las daria por inexistentes. Se devuelven aparte para que quien componga la
    /// respuesta pueda decir las dos cosas por separado.
    #[must_use]
    pub fn pegajosos_no_observados(&self) -> Vec<DireccionEnlace> {
        let mut sueltos: Vec<DireccionEnlace> = self
            .pegajoso
            .iter()
            .filter(|direccion| !self.volatil.contains_key(*direccion))
            .copied()
            .collect();

        sueltos.sort_unstable();
        sueltos
    }
}

/// Almacen de observacion.
///
/// Alimenta a la vez a [`ProveedorHuella`] y a [`ProveedorSegmento`]. RPT-018
/// §8.3 anticipaba que podrian dejar de ser independientes, y asi es: la VLAN de
/// un dispositivo se conoce por la misma observacion que su huella. Separarlos
/// en dos almacenes obligaria a mantener dos tablas con las mismas direcciones y
/// politicas de expulsion distintas, que es como se desincronizan.
#[derive(Debug, Clone, Default)]
pub struct AlmacenObservacion {
    volatil: HashMap<DireccionEnlace, EntradaVolatil>,
    pegajoso: Vec<DireccionEnlace>,
    reloj: u64,
    hay_perdida: bool,
}

impl AlmacenObservacion {
    /// Almacen vacio.
    #[must_use]
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Anota que la captura perdio tramas.
    ///
    /// Una vez cierto, permanece cierto: la huella acumulada ya esta incompleta
    /// y no se recompone porque la perdida cese. RPT-018 §4.
    pub const fn anotar_perdida(&mut self) {
        self.hay_perdida = true;
    }

    /// Indica si la vista de la red esta incompleta.
    #[must_use]
    pub const fn hay_perdida(&self) -> bool {
        self.hay_perdida
    }

    /// Indica si la mitad pegajosa se lleno.
    ///
    /// Mientras sea cierto, [`ProveedorSegmento`] devuelve error para todos los
    /// dispositivos: sin garantia de pegajosidad no se puede afirmar que un
    /// dispositivo sin marcar sea contenible.
    #[must_use]
    pub fn pegajoso_saturado(&self) -> bool {
        self.pegajoso.len() >= CAPACIDAD_PEGAJOSA
    }

    /// Numero de dispositivos en la mitad volatil.
    #[must_use]
    pub fn volatiles(&self) -> usize {
        self.volatil.len()
    }

    /// Numero de dispositivos con marca pegajosa.
    #[must_use]
    pub fn pegajosos(&self) -> usize {
        self.pegajoso.len()
    }

    /// Registra una observacion.
    ///
    /// Si el segmento admite criticos, la marca pegajosa se anota **antes** de
    /// cualquier expulsion: un dispositivo visto una sola vez en la VLAN clinica
    /// no puede perder esa marca por presion de tabla.
    pub fn observar(
        &mut self,
        mac: DireccionEnlace,
        protocolo: Option<Protocolo>,
        segmento: DeclaracionSegmento,
    ) {
        if segmento.admite_criticos() {
            self.anotar_pegajoso(mac);
        }

        self.reloj = self.reloj.saturating_add(1);

        if !self.volatil.contains_key(&mac) && self.volatil.len() >= CAPACIDAD_VOLATIL {
            self.expulsar_lote();
        }

        let entrada = self.volatil.entry(mac).or_insert_with(|| EntradaVolatil {
            protocolos: Vec::new(),
            segmento,
            visto_en: 0,
        });

        entrada.segmento = segmento;
        entrada.visto_en = self.reloj;

        if let Some(protocolo) = protocolo {
            if !entrada.protocolos.contains(&protocolo) {
                entrada.protocolos.push(protocolo);
            }
        }
    }

    fn anotar_pegajoso(&mut self, mac: DireccionEnlace) {
        if self.pegajoso.binary_search(&mac).is_err() && !self.pegajoso_saturado() {
            if let Err(posicion) = self.pegajoso.binary_search(&mac) {
                self.pegajoso.insert(posicion, mac);
            }
        }
    }

    /// Expulsa el lote mas antiguo de la mitad volatil.
    ///
    /// **No toca la mitad pegajosa.** Esa es la razon de que el almacen este
    /// partido.
    fn expulsar_lote(&mut self) {
        let mut edades: Vec<(u64, DireccionEnlace)> = self
            .volatil
            .iter()
            .map(|(mac, entrada)| (entrada.visto_en, *mac))
            .collect();

        edades.sort_unstable();

        for (_, mac) in edades.into_iter().take(EXPULSION_POR_LOTE) {
            self.volatil.remove(&mac);
        }
    }
}

impl ProveedorHuella for AlmacenObservacion {
    fn indicio(&self, mac: &DireccionEnlace) -> Result<Indicio, ErrorProveedor> {
        // Con perdida, cualquier conclusion sobre protocolos esta incompleta.
        // `Indeterminado` y no `SinIndicio`: la ausencia de indicio no puede
        // leerse como ausencia de riesgo (RPT-018 §4).
        if self.hay_perdida {
            return Ok(Indicio::Indeterminado);
        }

        let Some(entrada) = self.volatil.get(mac) else {
            // No observado todavia no es observado sin nada. Un dispositivo que
            // acaba de aparecer no ha tenido tiempo de delatarse.
            return Ok(Indicio::Indeterminado);
        };

        let sugerida = entrada
            .protocolos
            .iter()
            .filter_map(|protocolo| protocolo.sugiere())
            .next();

        Ok(match sugerida {
            Some(clase) => Indicio::SugiereCriticidad(clase),
            None => Indicio::SinIndicio,
        })
    }
}

impl ProveedorSegmento for AlmacenObservacion {
    fn historial(&self, mac: &DireccionEnlace) -> Result<HistorialSegmento, ErrorProveedor> {
        if self.pegajoso_saturado() {
            return Err(ErrorProveedor::FuenteInaccesible {
                fuente: "almacen-pegajoso-saturado".to_owned(),
            });
        }

        let actual = self
            .volatil
            .get(mac)
            .map_or(DeclaracionSegmento::NoDeclarado, |entrada| entrada.segmento);

        Ok(HistorialSegmento {
            actual,
            visto_en_segmento_critico: self.pegajoso.binary_search(mac).is_ok(),
        })
    }
}
