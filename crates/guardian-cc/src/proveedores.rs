//! Proveedores de evidencia para la clasificacion.
//!
//! RPT-010, PA-24.
//!
//! # Que resuelve este modulo
//!
//! RPT-009 dejo `clasificar()` como una funcion sin entradas: sabe combinar la
//! evidencia pero nadie la produce. Aqui se define **el contrato de esos
//! productores**, no su implementacion concreta, que depende de captura en
//! `eje-red` y de una base de datos OUI todavia por decidir.
//!
//! # Cuatro correcciones sobre la especificacion recibida
//!
//! ## 1. Ningun proveedor devuelve `bool`
//!
//! Un `Result<bool, _>` obliga a quien llama a convertir el error en `true` o en
//! `false`, y `unwrap_or(false)` es la conversion que alguien escribira. Ese
//! `false` significa «no es critico», que es exactamente lo que ninguna fuente
//! inferida puede afirmar (RPT-009 §3).
//!
//! Los proveedores devuelven tipos que incluyen **«no puedo saberlo»** como
//! variante legitima, para que no exista la tentacion de coercionarlo.
//!
//! ## 2. El marcado lleva clase, no un booleano
//!
//! `es_critico: bool` no distingue entre soporte vital, seguridad funcional y
//! camino de gestion. Las tres clases existen desde RPT-008 y el veredicto las
//! nombra.
//!
//! ## 3. La firma por entrada no protege contra la supresion
//!
//! Es la correccion importante. Si cada marcado lleva su propia firma, un
//! atacante que **borre** la entrada «esta bomba es soporte vital» no rompe
//! ninguna firma: las que quedan verifican perfectamente. El dispositivo pasa a
//! ser contenible y nada protesta.
//!
//! El inventario se ancla **completo** a una raiz Merkle, que es justo lo que
//! `eje-almacen` ya construye. Verificar un marcado exige ademas una prueba de
//! inclusion contra esa raiz.
//!
//! ## 4. Las interfaces sin estado no pueden ver el equipo rodante
//!
//! La especificacion pedia degradar a ambiguo cuando un dispositivo cambia de
//! segmento, con interfaces de la forma `fn(&mac) -> respuesta`. Una consulta
//! puntual sin memoria no puede saber que hubo un cambio. Hace falta historial,
//! y por eso [`ProveedorSegmento`] expone la observacion acumulada y no solo la
//! actual.

use crate::ClaseExcluida;
use crate::clasificacion::{
    Clasificacion, DeclaracionSegmento, Evidencia, MarcadoDispositivo, MotivoAmbiguedad, clasificar,
};

/// Direccion de capa de enlace.
///
/// Es un identificador **debil**: se falsifica trivialmente. Ver la advertencia
/// de [`ProveedorInventario`] sobre lo que eso implica.
pub type DireccionEnlace = [u8; 6];

/// Fallo de un proveedor.
///
/// No existe variante que signifique «no es critico». Un proveedor que no puede
/// responder devuelve la variante de desconocimiento de su tipo de respuesta, no
/// un error que alguien convierta en permiso.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorProveedor {
    /// La firma del inventario no verifica.
    #[error("la firma del inventario no verifica: {detalle}")]
    FirmaInvalida {
        /// Que falló, para el registro forense.
        detalle: String,
    },

    /// El marcado existe pero no se pudo probar su pertenencia al inventario.
    ///
    /// Es el sintoma de un intento de supresion: la entrada dice una cosa y la
    /// raiz anclada no la respalda.
    #[error("el marcado no prueba pertenencia a la raiz anclada")]
    InclusionNoProbada,

    /// La fuente de datos no esta disponible o esta corrupta.
    #[error("fuente '{fuente}' inaccesible")]
    FuenteInaccesible {
        /// Nombre de la fuente.
        fuente: String,
    },
}

/// Respuesta de una fuente **inferida**.
///
/// Deliberadamente no tiene variante «no es critico»: ver RPT-009 §3. Una huella
/// pasiva o un OUI pueden sugerir criticidad o no saber, nunca descartarla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indicio {
    /// La fuente apunta a esta clase.
    SugiereCriticidad(ClaseExcluida),
    /// La fuente no observo nada relevante.
    ///
    /// **No** significa «no es critico». Significa que esta fuente no aporta.
    SinIndicio,
    /// La fuente no pudo consultarse.
    ///
    /// Se distingue de [`Self::SinIndicio`] porque colapsarlas repetiria el
    /// defecto que RPT-006 §4 documenta: una base de datos caida se leeria como
    /// ausencia de riesgo.
    Indeterminado,
}

impl Indicio {
    /// Clase sugerida, si la fuente sugiere alguna.
    #[must_use]
    pub const fn clase(self) -> Option<ClaseExcluida> {
        match self {
            Self::SugiereCriticidad(clase) => Some(clase),
            Self::SinIndicio | Self::Indeterminado => None,
        }
    }

    /// Indica si la consulta fue concluyente.
    #[must_use]
    pub const fn es_concluyente(self) -> bool {
        !matches!(self, Self::Indeterminado)
    }
}

/// Marcado administrativo verificado contra la raiz anclada.
///
/// Solo se construye tras comprobar firma **y** prueba de inclusion. Un valor de
/// este tipo es, por construccion, un marcado que pertenece al inventario
/// firmado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarcadoVerificado {
    /// Clase declarada. `None` significa «declarado no critico».
    pub clase: Option<ClaseExcluida>,
    /// Instante de emision, en segundos desde la epoca.
    pub emitido_en: u64,
    /// Vigencia declarada, en dias.
    pub vigencia_dias: u32,
}

impl MarcadoVerificado {
    /// Indica si el marcado sigue vigente en el instante dado.
    ///
    /// # Politica de reloj
    ///
    /// Un agente Local-First puede tener el reloj desviado. Ante duda, este
    /// metodo declara **caducado**, no vigente: un marcado caducado degrada a
    /// ambiguo y escala a un humano, mientras que uno indebidamente vigente
    /// permitiria contener un equipo critico.
    ///
    /// Por eso un `ahora` anterior a la emision —reloj atrasado, o marcado con
    /// fecha futura— tambien cuenta como caducado en lugar de tratarse como
    /// «aun no empieza».
    #[must_use]
    pub const fn vigente_en(&self, ahora: u64) -> bool {
        if ahora < self.emitido_en {
            return false;
        }

        let transcurrido = ahora - self.emitido_en;
        let vigencia_segundos = (self.vigencia_dias as u64).saturating_mul(86_400);

        transcurrido <= vigencia_segundos
    }
}

/// Observacion acumulada de los segmentos donde se ha visto un dispositivo.
///
/// El historial es lo que permite detectar el equipo rodante. Una consulta
/// puntual no puede.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistorialSegmento {
    /// Naturaleza del segmento donde se observa ahora.
    pub actual: DeclaracionSegmento,
    /// Si alguna vez se observo en un segmento que admite criticos.
    ///
    /// Una vez cierto, permanece cierto hasta que un humano lo limpie. Es la
    /// **ambiguedad pegajosa**: un carro de telemedicina que pasa por la VLAN
    /// clinica y aparece luego en la administrativa no debe volverse contenible
    /// por haberse movido.
    pub visto_en_segmento_critico: bool,
}

impl HistorialSegmento {
    /// Declaracion efectiva, aplicando la ambiguedad pegajosa.
    #[must_use]
    pub const fn declaracion_efectiva(&self) -> DeclaracionSegmento {
        if self.visto_en_segmento_critico {
            return DeclaracionSegmento::PuedeAlojarCriticos;
        }
        self.actual
    }
}

/// Resuelve el OUI de una direccion de enlace.
///
/// # Limitacion conocida
///
/// Muchos equipos medicos e industriales usan modulos de red comerciales, con lo
/// que el OUI identifica al fabricante del modulo y no al del equipo. Esta fuente
/// aporta indicios, jamas descartes (RPT-009 §3).
pub trait ProveedorOui: Send + Sync {
    /// Indicio derivado del prefijo de fabricante.
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorProveedor::FuenteInaccesible`] si la base no esta
    /// disponible. Quien llama debe tratarlo como [`Indicio::Indeterminado`], no
    /// como ausencia de riesgo.
    fn indicio(&self, mac: &DireccionEnlace) -> Result<Indicio, ErrorProveedor>;
}

/// Evalua la huella de protocolo observada pasivamente.
pub trait ProveedorHuella: Send + Sync {
    /// Indicio derivado de los protocolos observados.
    ///
    /// La observacion es **pasiva**: no se emiten sondas. En perfil OT eso no es
    /// una preferencia sino un requisito (RPT-002 §9.2).
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorProveedor::FuenteInaccesible`] si la captura no esta
    /// operativa.
    fn indicio(&self, mac: &DireccionEnlace) -> Result<Indicio, ErrorProveedor>;
}

/// Lee el inventario de marcados firmados.
///
/// # Advertencia sobre la identidad
///
/// El inventario se indexa por direccion de enlace, que se falsifica
/// trivialmente. El fallo es **asimetrico** y conviene tenerlo escrito: quien
/// suplanta la MAC de un equipo critico no consigue que se contenga a un tercero
/// por error, consigue **volverse incontenible**.
///
/// Es decir, la lista de exclusion es por construccion un vector de evasion. No
/// se corrige haciendo infalsificable la MAC, porque no puede serlo. Se mitiga
/// con identidad por certificado 802.1X donde exista, y sobre todo con que la
/// prohibicion **nunca sea silenciosa**: una amenaza sobre un equipo que no
/// podemos contener es lo mas urgente que este producto puede comunicar.
pub trait ProveedorInventario: Send + Sync {
    /// Marcado vigente para un dispositivo, ya verificado.
    ///
    /// La implementacion debe comprobar **firma y prueba de inclusion** contra
    /// la raiz anclada antes de devolver `Ok(Some(..))`. Verificar solo la firma
    /// de la entrada deja pasar el ataque de supresion descrito en el modulo.
    ///
    /// # Errores
    ///
    /// [`ErrorProveedor::FirmaInvalida`] si la firma no verifica,
    /// [`ErrorProveedor::InclusionNoProbada`] si la entrada no pertenece a la
    /// raiz anclada, [`ErrorProveedor::FuenteInaccesible`] si el inventario no
    /// esta disponible.
    fn marcado(&self, mac: &DireccionEnlace) -> Result<Option<MarcadoVerificado>, ErrorProveedor>;
}

/// Aporta el contexto de segmento y su historial.
pub trait ProveedorSegmento: Send + Sync {
    /// Historial de segmentos observado para un dispositivo.
    ///
    /// # Errores
    ///
    /// Devuelve [`ErrorProveedor::FuenteInaccesible`] si el registro no esta
    /// disponible.
    fn historial(&self, mac: &DireccionEnlace) -> Result<HistorialSegmento, ErrorProveedor>;
}

/// Conjunto de proveedores que alimentan la clasificacion.
pub struct Proveedores<'a> {
    /// Inventario firmado. Fuente declarativa.
    pub inventario: &'a dyn ProveedorInventario,
    /// Registro de segmentos. Fuente declarativa.
    pub segmento: &'a dyn ProveedorSegmento,
    /// Prefijo de fabricante. Fuente inferida.
    pub oui: &'a dyn ProveedorOui,
    /// Huella de protocolo. Fuente inferida.
    pub huella: &'a dyn ProveedorHuella,
}

/// Reune la evidencia disponible sobre un dispositivo.
///
/// # Como falla cada fuente
///
/// La regla no es «ante cualquier fallo, bloquear»: eso haria el producto
/// fragil, porque bastaria tumbar la captura para inutilizar la contencion en
/// toda la red. La regla se deriva de la asimetria de RPT-009 §3.
///
/// | Fuente | Naturaleza | Si falla |
/// |---|---|---|
/// | Inventario | declarativa | **bloquea** |
/// | Segmento | declarativa | **bloquea** |
/// | OUI | inferida | se ignora |
/// | Huella | inferida | se ignora |
///
/// El permiso para contener procede **siempre** de una fuente declarativa. La
/// inferencia nunca lo concede, asi que su ausencia tampoco puede retirarlo. Un
/// fallo declarativo, en cambio, significa que no sabemos si el dispositivo esta
/// marcado como critico, y eso si obliga a escalar.
///
/// Una firma invalida o una inclusion no probada **no** son «marcado ausente»:
/// son un intento de manipulacion del inventario y producen
/// [`MotivoAmbiguedad::EvidenciaNoVerificable`].
///
/// # Errores
///
/// Devuelve el motivo de ambiguedad cuando una fuente declarativa no responde o
/// no verifica.
pub fn reunir_evidencia(
    proveedores: &Proveedores<'_>,
    mac: &DireccionEnlace,
    ahora: u64,
) -> Result<Evidencia, MotivoAmbiguedad> {
    let historial = proveedores
        .segmento
        .historial(mac)
        .map_err(|_| MotivoAmbiguedad::EvidenciaNoVerificable)?;

    let marcado = proveedores
        .inventario
        .marcado(mac)
        .map_err(|_| MotivoAmbiguedad::EvidenciaNoVerificable)?;

    // Fuentes inferidas: un fallo se lee como ausencia de indicio, nunca como
    // ausencia de riesgo. La diferencia importa porque el permiso no vino de
    // aqui.
    let indicio_oui = proveedores
        .oui
        .indicio(mac)
        .unwrap_or(Indicio::Indeterminado);
    let indicio_huella = proveedores
        .huella
        .indicio(mac)
        .unwrap_or(Indicio::Indeterminado);

    let inferencia = indicio_huella.clase().or_else(|| indicio_oui.clase());

    Ok(Evidencia {
        marcado: marcado.map(|marcado| MarcadoDispositivo {
            clase: marcado.clase,
            vigente: marcado.vigente_en(ahora),
        }),
        segmento: historial.declaracion_efectiva(),
        inferencia,
    })
}

/// Clasifica un dispositivo consultando a los proveedores.
///
/// Es la union de [`reunir_evidencia`] y [`clasificar`]: si la evidencia no se
/// puede reunir, el resultado es ambiguo con el motivo correspondiente.
#[must_use]
pub fn clasificar_con_proveedores(
    proveedores: &Proveedores<'_>,
    mac: &DireccionEnlace,
    ahora: u64,
) -> Clasificacion {
    match reunir_evidencia(proveedores, mac, ahora) {
        Ok(evidencia) => clasificar(&evidencia),
        Err(motivo) => Clasificacion::Ambiguo { motivo },
    }
}
