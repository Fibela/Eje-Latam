//! Cargas útiles de los canales del puente.
//!
//! RPT-006, PA-21. Las formas se declaran en `contrato-ipc.toml`; aquí se
//! implementan, y `pruebas.rs` comprueba que ambas coincidan.
//!
//! # `deny_unknown_fields` no es opcional
//!
//! Sin él, un mensaje con campos sobrantes se acepta en silencio. Eso convierte
//! un cambio de contrato no coordinado —o un renderer comprometido enviando
//! basura extra— en algo que el deserializador aprueba.
//!
//! Es la mitad de la asimetría documentada en el manifiesto: **el rigor de Rust
//! vive en el deserializador**. TypeScript no tiene equivalente natural, y por
//! eso su rigor vive en la prueba de paridad.
//!
//! # Nombres en el cable
//!
//! camelCase, porque el extremo TypeScript los consume tal cual. El mapeo se
//! hace con `rename_all`, no renombrando campos en Rust: los identificadores
//! siguen la convención del lenguaje.

use serde::{Deserialize, Serialize};

/// Perfil del segmento vigilado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerfilSegmento {
    /// Red corporativa de propósito general.
    Corporativo,
    /// Red industrial u hospitalaria.
    Ot,
}

/// Lo que se sabe de la clase de un dispositivo, **con su procedencia dentro**.
///
/// RPT-089, PA-139 (reabierto).
///
/// # Es el espejo de `Clasificacion`, no un modelo aparte
///
/// La primera version de este enumerado se diseno sobre lo que parecia razonable
/// y **afirmaba cosas que el dominio se niega a afirmar**. Tenia
/// `InferidaSoporteVital`, y `guardian_cc::clasificacion::clasificar` nunca
/// devuelve eso: cuando la huella sugiere criticidad sin marcado que la
/// respalde, declara `Ambiguo { InferenciaSugiereCriticidad }`. **Una fuente
/// inferida no afirma una clase; levanta la mano.**
///
/// Cada variante de aqui corresponde a un resultado alcanzable de `clasificar`,
/// uno a uno. Ni mas —seria inventar dato— ni menos —seria colapsar estados.
///
/// # El que faltaba, y era el que mas importa
///
/// [`Self::DeclaradaNoCritica`] es «no critico, **y hay un humano que lo
/// firma**». Es el unico estado que permite accion automatica
/// (`Clasificacion::permite_accion_automatica`). En la version anterior se
/// habria leido como «sin indicio», que significa lo contrario: que nada apunta.
///
/// # Y las cuatro ambiguedades no son una
///
/// «Su marcado caduco», «el marcado y la huella se contradicen», «la huella
/// sugiere y no hay marcado» y «nadie declaro el segmento» mandan a mirar sitios
/// distintos. Colapsarlas en un `ambigua` generico le quitaria al operador lo
/// unico que le dice por donde empezar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaseConocida {
    /// Marcado firmado y vigente: soporte vital.
    DeclaradaSoporteVital,
    /// Marcado firmado y vigente: seguridad funcional.
    DeclaradaSeguridadFuncional,
    /// Marcado firmado y vigente: camino de gestion.
    DeclaradaCaminoDeGestion,
    /// Marcado firmado y vigente que declara **ausencia** de criticidad.
    ///
    /// Unico estado con un humano responsable detras que permite contener sin
    /// intervencion. No confundir con [`Self::SinIndicio`], que es lo contrario.
    DeclaradaNoCritica,
    /// Sin marcado, sin huella que sugiera nada, y el segmento esta declarado
    /// libre de criticos.
    ///
    /// La responsabilidad humana esta al nivel del segmento, no del equipo.
    SegmentoDeclaradoSinCriticos,
    /// Existio un marcado y su vigencia expiro. Se degrada a ausencia.
    AmbiguaMarcadoCaducado,
    /// El marcado dice una cosa y la huella observada dice otra.
    AmbiguaConflictoEntreFuentes,
    /// La huella apunta a un equipo critico y **no hay marcado que lo confirme**.
    ///
    /// No dice cual: la inferencia no afirma clases.
    AmbiguaInferenciaSugiereCriticidad,
    /// Sin marcado, en un segmento que admite criticos o sin declarar.
    AmbiguaSegmentoPuedeAlojarCriticos,
    /// Una fuente **declarativa** no respondio, o el inventario no verifica.
    ///
    /// # No es «no se pudo consultar»
    ///
    /// Una firma invalida o una inclusion no probada indican **manipulacion del
    /// inventario**, no que el dispositivo carezca de marcado (RPT-010). Se
    /// separo de las demas porque acusar de manipulacion o no acusar no es un
    /// matiz de presentacion.
    AmbiguaEvidenciaNoVerificable,
    /// El agente **no formo veredicto**.
    ///
    /// No es ninguna de las cinco ambiguedades: aquellas son juicios sobre la
    /// evidencia, y esto es la ausencia de juicio. Corresponde a
    /// `Clasificacion::NoClasificado`, que `guardian-cc` mantiene deliberadamente
    /// inalcanzable para que nadie asuma que la evidencia siempre llega.
    ///
    /// No acusa a nadie ni absuelve a nadie.
    Indeterminada,
}

/// Lo que el administrador declaro del segmento donde se vio al equipo.
///
/// Espejo de `guardian_cc::clasificacion::DeclaracionSegmento`. Se duplica el
/// tipo, no se importa: `eje-ipc` depende solo de `thiserror` y `serde` a
/// proposito, y traer `guardian-cc` invertiria esa dependencia. La traduccion
/// vive en `eje-agente`, como `perfil_en_el_cable` (RPT-081).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeclaracionSegmento {
    /// El administrador declara que el segmento no aloja equipos criticos.
    SinDispositivosCriticos,
    /// Segmento clinico, de planta o similar.
    PuedeAlojarCriticos,
    /// Nadie declaro nada. Se trata como [`Self::PuedeAlojarCriticos`].
    NoDeclarado,
}

/// Estado resumido del demonio local. Respuesta de `obtener-estado-agente`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EstadoAgente {
    /// Versión del agente en ejecución.
    pub version: String,
    /// Perfil del segmento vigilado.
    pub perfil: PerfilSegmento,
    /// Si la respuesta automática está habilitada según vigencia de reglas.
    pub respuesta_automatica: bool,
}

/// Dispositivo IoT/OT observado. Elemento de `obtener-inventario`.
///
/// RPT-088, PA-139. **Lleva evidencia, no juicios.**
///
/// # Que se fue de aqui, y por que
///
/// - `identificador` era la MAC serializada, igual que `direccionEnlace`. Un
///   campo que finge ser una abstraccion y es el mismo dato.
/// - `postura` (`conforme|anomalo|contenido`) **no tenia productor en ninguna
///   parte**: su unica aparicion en Rust era un dato de prueba, y en TypeScript
///   si tenia consumidor. Un tipo con consumidor y sin productor es la clase de
///   defecto dominante del proyecto, aqui a nivel de campo.
///
/// El agente sabe `Indicio`, `DeclaracionSegmento` y la marca de segmento
/// critico. Eso no es una postura: son las evidencias con las que se forma una.
/// El agente es testigo; el juicio lo compone VIS-04, donde cambiar la regla no
/// exige recompilar el sensor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodoInventario {
    /// Direccion de capa de enlace, en notacion de MAC. Es la clave.
    pub direccion_enlace: String,
    /// Clase y respaldo, en un solo valor.
    pub clase: ClaseConocida,
    /// Lo que el administrador declaro del segmento.
    pub declaracion_segmento: DeclaracionSegmento,
    /// Se le vio en un segmento que admite criticos.
    ///
    /// **No es contencion.** El agente no contiene a nadie (RPT-020).
    pub visto_en_segmento_critico: bool,
    /// Protocolos industriales observados, en el orden en que se anotaron.
    pub protocolos_observados: Vec<String>,
    //
    // NO lleva el reloj interno del almacen (`VistaNodo::visto_en`). Es un
    // contador de vueltas, no una marca de tiempo: en pantalla se leeria como
    // una fecha y no lo es. Que no pueda colarse lo sujeta `CAMPOS_*`, atado a
    // este struct por desestructuracion exhaustiva.
}

/// Ocupación de la Bóveda Aislada. Respuesta de `obtener-estado-boveda`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EstadoBoveda {
    /// Bytes ocupados por la cola de eventos pendientes.
    pub usado_bytes: u64,
    /// Límite configurado en bytes.
    pub limite_bytes: u64,
    /// Eventos pendientes de reconciliación.
    pub eventos_pendientes: u64,
}

/// Consulta dirigida al sandbox del analista. Petición de `consultar-sandbox`.
///
/// Opera **solo contra ALM-02**. El registro de evidencia ALM-01 no es
/// alcanzable desde la interfaz (RPT-002 §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeticionConsulta {
    /// Sentencia SQL a ejecutar contra ALM-02.
    pub sentencia: String,
}

/// Filas devueltas por el sandbox. Respuesta de `consultar-sandbox`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultadoConsulta {
    /// Nombres de columna devueltos.
    pub columnas: Vec<String>,
    /// Filas devueltas, en el orden de `columnas`.
    pub filas: Vec<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Descripción declarativa, para la prueba de paridad
// ---------------------------------------------------------------------------
//
// Estas constantes NO son una tercera declaración independiente: `pruebas.rs`
// las ata a los structs mediante desestructuración exhaustiva, que deja de
// compilar si se añade o se quita un campo. La cadena queda:
//
//   contrato-ipc.toml  <-- prueba de paridad -->  CAMPOS_*
//   CAMPOS_*           <-- compilador        -->  struct

/// Campos de [`EstadoAgente`], en orden y con su tipo del manifiesto.
pub const CAMPOS_ESTADO_AGENTE: [(&str, &str); 3] = [
    ("version", "texto"),
    ("perfil", "enumerado"),
    ("respuestaAutomatica", "booleano"),
];

/// Campos de [`NodoInventario`].
pub const CAMPOS_NODO_INVENTARIO: [(&str, &str); 5] = [
    ("direccionEnlace", "texto"),
    ("clase", "enumerado"),
    ("declaracionSegmento", "enumerado"),
    ("vistoEnSegmentoCritico", "booleano"),
    ("protocolosObservados", "lista<texto>"),
];

/// Campos de [`EstadoBoveda`].
pub const CAMPOS_ESTADO_BOVEDA: [(&str, &str); 3] = [
    ("usadoBytes", "entero"),
    ("limiteBytes", "entero"),
    ("eventosPendientes", "entero"),
];

/// Campos de [`PeticionConsulta`].
pub const CAMPOS_PETICION_CONSULTA: [(&str, &str); 1] = [("sentencia", "texto")];

/// Campos de [`ResultadoConsulta`].
pub const CAMPOS_RESULTADO_CONSULTA: [(&str, &str); 2] = [
    ("columnas", "lista<texto>"),
    ("filas", "lista<lista<texto>>"),
];

// ---------------------------------------------------------------------------
// Alertas — RPT-019
// ---------------------------------------------------------------------------

/// Clase de suceso de alerta.
///
/// # Por que hay una sola variante
///
/// De los tres centinelas de RPT-019 §1, **solo uno es un suceso**: los otros
/// dos son condiciones y viajan en [`Condiciones`]. Anadir aqui variantes para
/// ellos convertiria un estado persistente en una lluvia de sucesos repetidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClaseAlerta {
    /// Se detecto una amenaza sobre un dispositivo que no puede contenerse.
    ///
    /// Lo mas urgente que este producto puede comunicar: no existe accion
    /// automatica posible y la unica respuesta es humana (RPT-010 §6.1).
    AmenazaIncontenible,
}

/// Punto del registro desde el que se piden las alertas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PeticionAlertas {
    /// Numero de asiento desde el que continuar, exclusivo.
    pub desde_asiento: u64,
}

/// Suceso de alerta ya anexado a ALM-01.
///
/// Lleva su numero de asiento para que quien consulte pueda continuar desde
/// donde lo dejo sin recibir lo mismo dos veces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SucesoAlerta {
    /// Asiento de ALM-01 que la contiene.
    pub asiento: u64,
    /// Que ocurrio.
    pub clase: ClaseAlerta,
    /// Dispositivo implicado.
    pub dispositivo: String,
    /// Contexto para el operador.
    pub detalle: String,
}

/// Estados degradados vigentes.
///
/// # Condiciones, no sucesos
///
/// Son verdaderas hasta que alguien interviene, asi que no se anexan al
/// registro: se consultan. Anotarlas repetidamente inundaria ALM-01 con la misma
/// noticia (RPT-019 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Condiciones {
    /// Habia inventario y ya no esta (RPT-017 §2).
    pub inventario_suprimido: bool,
    /// El inventario esta presente y no supera la verificacion.
    pub inventario_no_verifica: bool,
    /// La mitad pegajosa del almacen se lleno (RPT-018 §6).
    pub observacion_saturada: bool,
    /// La captura perdio tramas y la vista de la red esta incompleta.
    pub captura_con_perdida: bool,
    /// **No hay captura en absoluto**: este sensor no esta observando.
    ///
    /// RPT-047, PA-81.
    ///
    /// # Por que no es `captura_con_perdida`
    ///
    /// Aquella dice «observo y se me escapan tramas»; la vista es **incompleta**.
    /// Esta dice que no hay vista. Colapsarlas seria afirmar que se ve mal
    /// cuando no se ve nada, y es como un operador concluye que en ese segmento
    /// no paso nada (RPT-036 §6).
    ///
    /// # Por que el motivo no viaja aqui
    ///
    /// Privilegios, interfaz inexistente e interfaz desaparecida en marcha son
    /// tres remedios distintos, y el motivo importa. Pero `Condiciones` describe
    /// **lo que es verdad ahora**, con una forma uniforme que seis sitios
    /// comprueban; un texto que carece de sentido mientras la condicion es falsa
    /// no pertenece a esa forma.
    ///
    /// El motivo viaja donde se diagnostica: en el asiento de ALM-01 que anota
    /// la transicion y en la linea de syslog. VIS-04 muestra que no se observa;
    /// el porque esta a un clic, en el registro.
    ///
    /// # Y por que esto no puede parecer un agente sano
    ///
    /// Un proceso muerto lo reinicia el supervisor y alguien se entera. Un
    /// agente vivo que no observa puede pasar por bueno durante meses. Por eso
    /// esta condicion **se emite tambien por syslog con la gravedad mas alta**:
    /// que el sensor deje de mirar es un incidente, no un aviso.
    pub captura_no_disponible: bool,
    /// El almacen exige una accion del administrador, sin indicio de ataque.
    ///
    /// # Por que no cabe en las otras cuatro
    ///
    /// Cubre `FormatoObsoleto` (RPT-022) y `SinClaveAprovisionada` (RPT-024):
    /// dos estados que **exigen alerta y no son manipulacion**. Su remedio es
    /// reemitir o aprovisionar, no responder a un incidente.
    ///
    /// Mezclarlos con la supresion o la firma rota produciria exactamente la
    /// fatiga de alertas que la Fase 1 de PA-45 existia para evitar: un operador
    /// que aprendio a ignorar esta aviso la ignorara el dia que sea un ataque.
    pub accion_administrativa: bool,
    /// El colector de syslog no responde y la alerta no sale del equipo.
    ///
    /// # Una de las dos condiciones que no se pueden emitir
    ///
    /// Las demas viajan tambien por syslog hacia el SIEM del cliente. Esta no:
    /// emitirla exigiria el canal que acaba de fallar. Llega solo por este
    /// puente, que es donde VIS-04 la consulta.
    ///
    /// Existe porque un canal de alertas caido y uno silencioso son
    /// indistinguibles desde fuera, y RPT-006 §4 obliga a distinguirlos.
    ///
    /// La otra es [`Self::sin_colector`], por la misma imposibilidad.
    pub salida_no_disponible: bool,
    /// Este agente **no tiene colector configurado**: no emite ni late.
    ///
    /// RPT-054 §4, PA-109.
    ///
    /// # Por que no es lo mismo que `salida_no_disponible`
    ///
    /// Aquella dice que el colector existe y ahora mismo no responde: es una
    /// averia, se resuelve sola cuando la red vuelve, y se responde llamando a
    /// quien mantiene el SIEM.
    ///
    /// Esta dice que **nunca hubo colector**. No se resuelve sola, no es una
    /// averia y no se responde: se configura. Colapsarlas mandaria al operador
    /// a investigar una caida de red que no existe, y al reves haria pasar una
    /// caida real por una instalacion a medias.
    ///
    /// # La segunda condicion no emisible, y por el mismo motivo
    ///
    /// Un agente sin colector no puede avisar de que no tiene colector: el aviso
    /// viajaria por el canal que no existe. Por eso llega solo por este puente,
    /// y por eso el instalador y `journald` la declaran ademas (RPT-054 §4).
    ///
    /// # Por que es una condicion y no un dato de configuracion
    ///
    /// Porque es verdadera hasta que alguien interviene, que es la definicion
    /// exacta de RPT-019 §2. Y porque el tecnico que va a la planta a averiguar
    /// por que un sensor no aparece en la sala tiene que verlo **en el tablero**,
    /// no saltando de VIS-04 a los diarios del sistema.
    pub sin_colector: bool,
    /// **La escucha local no esta abierta**: ninguna consola puede conectarse.
    ///
    /// RPT-070, PA-125. Se observo ocurriendo, no razonando: en `systemd` real,
    /// el conjunto acotado de capacidades impedia asignar el grupo al socket y el
    /// sensor arrancaba sin escucha (RPT-069 §3). Las diez condiciones de
    /// entonces decian, todas, que estaba sano.
    ///
    /// # Esta SI se emite, y es la clave
    ///
    /// [`Self::salida_no_disponible`] y [`Self::sin_colector`] no viajan porque
    /// describen **el canal de syslog mismo**: contarlas exigiria el canal que
    /// falla. Esta describe **el otro canal**.
    ///
    /// Cuando la escucha local cae, syslog es justamente lo que sigue
    /// funcionando, y es el unico camino por el que la sala puede enterarse. Lo
    /// que no puede contarlo es la consola, que es lo que no conecta.
    ///
    /// # Por que no cabe en `accion_administrativa`
    ///
    /// Aquella dice «el almacen exige que alguien intervenga» y su remedio es
    /// reemitir o aprovisionar. Esta dice que el sensor **funciona y es
    /// inalcanzable**: sigue observando, registrando y emitiendo, y nadie puede
    /// preguntarle nada. Son dos visitas distintas a la planta.
    ///
    /// # Un sensor vivo e inalcanzable es peor que uno caido
    ///
    /// Al caido lo reinicia el supervisor y alguien se entera. Este pasa por
    /// bueno mientras dure. Es el mismo argumento de
    /// [`Self::captura_no_disponible`], aplicado al puente en lugar de a la
    /// captura.
    pub escucha_no_disponible: bool,
    /// **Este sensor corre sin configuracion firmada.**
    ///
    /// RPT-074, PA-79. Sus parametros salen de la linea de ordenes, asi que quien
    /// controle el arranque puede alargar la ventana de silencio que la sala
    /// vigila, apuntar el sensor a otro segmento o cambiarle el nombre.
    ///
    /// # Por que no es un fallo
    ///
    /// Es un estado legitimo de desarrollo y de forense: el agente lanzado a mano
    /// es la herramienta con la que se observaron PA-123 y PA-125. Lo que no
    /// puede es pasar desapercibido.
    ///
    /// # Y por que no es un mock
    ///
    /// Un mock **se hace pasar por** lo real y el defecto es la
    /// indistinguibilidad. Esta condicion existe justamente para que las dos
    /// situaciones no se confundan. Si el agente arrancara con argumentos y
    /// **callara**, entonces si seria de esa familia.
    ///
    /// # El riesgo que vigila esta condicion no es tecnico
    ///
    /// Es que el estado degradado se vuelva el normal: si desplegar sin firmar
    /// fuera comodo, todo el mundo desplegaria sin firmar y se aprenderia a
    /// ignorarla. La defensa es estructural —la unidad no pasa configuracion, asi
    /// que un despliegue sin fichero firmado no captura nada— y esta condicion es
    /// como se ve desde fuera.
    pub configuracion_sin_firmar: bool,
    /// **Hay configuracion firmada y el agente NO la acepta.**
    ///
    /// RPT-074, PA-79. Distinta de [`Self::configuracion_sin_firmar`] y no por
    /// matiz: aquella dice «todavia no se ha aprovisionado» y esta dice «hay una
    /// y no vale». Colapsarlas mandaria a aprovisionar cuando lo que hay es
    /// alguien que toco el fichero.
    ///
    /// Es la distincion de [`Self::inventario_suprimido`] frente a
    /// [`Self::inventario_no_verifica`], copiada tal cual.
    ///
    /// # Por que NO cuenta como manipulacion
    ///
    /// Aunque la firma rota apunte a que alguien lo toco, las otras causas no:
    /// una configuracion emitida para otra maquina, una clave rotada, un fichero
    /// corrupto por disco. Presentarlas todas como incidente mandaria al operador
    /// a respuesta a incidentes por un error de despliegue, que es la fatiga de
    /// alertas que PA-45 §1 existe para evitar.
    ///
    /// Viaja con gravedad alta y sin acusar a nadie, como
    /// [`Self::registro_saturado`]: no dice «esto es un ataque», dice «esto no
    /// puede esperar al lunes».
    ///
    /// # El motivo no viaja aqui
    ///
    /// Firma invalida, maquina ajena y version desconocida son tres remedios
    /// distintos. `Condiciones` describe **lo que es verdad ahora** con una forma
    /// uniforme; el motivo viaja donde se diagnostica —el diario y la linea de
    /// syslog—, igual que en [`Self::captura_no_disponible`].
    pub configuracion_no_verifica: bool,
    /// El registro de evidencia esta lleno y **ya no admite alertas**.
    ///
    /// RPT-039 §1, PA-72.
    ///
    /// # Por que es la mas grave de las siete
    ///
    /// Las otras seis dicen que algo se ve peor. Esta dice que **este sensor ha
    /// dejado de registrar amenazas**: la alerta se calcula, no cabe en ALM-01 y
    /// se pierde. Un vigilante que no toma nota no es un vigilante degradado, es
    /// uno que no esta.
    ///
    /// No es manipulacion: nadie toco nada, el agente trabajo demasiado tiempo.
    /// Antes de PA-72 se disfrazaba de manipulacion, que era lo peor de los dos
    /// mundos — mandaba al operador a investigar un ataque inexistente mientras
    /// el sensor seguia sin registrar.
    pub registro_saturado: bool,
    /// Hay alertas anexadas que **solo viven en memoria**.
    ///
    /// RPT-044, PA-69. La escritura a disco fallo y el registro en memoria va por
    /// delante del que hay en el fichero. Las alertas existen y estan completas;
    /// lo que no esta es su durabilidad.
    ///
    /// # Se apaga sola, y por eso no basta
    ///
    /// Es una condicion: deja de ser cierta en cuanto el disco vuelve y el
    /// reintento escribe. Un fallo de dos segundos puede no verse en ninguna
    /// consulta. La constancia duradera de que hubo un tramo en riesgo es el
    /// asiento `persistencia-restablecida`, que se anexa al recuperar.
    pub evidencia_en_riesgo: bool,
}

impl Condiciones {
    /// Indica si alguna condicion degradada esta vigente.
    #[must_use]
    pub const fn hay_degradacion(&self) -> bool {
        self.inventario_suprimido
            || self.inventario_no_verifica
            || self.observacion_saturada
            || self.captura_con_perdida
            || self.captura_no_disponible
            || self.accion_administrativa
            || self.salida_no_disponible
            || self.registro_saturado
            || self.evidencia_en_riesgo
            // RPT-070, PA-125. Un sensor al que nadie puede preguntar esta
            // degradado aunque todo lo demas funcione: la mitad del producto que
            // el tecnico usa no existe.
            || self.escucha_no_disponible
            // RPT-074, PA-79. Las dos degradan: un sensor cuyos parametros no
            // estan firmados obedece a quien controle el arranque, y uno cuya
            // configuracion no verifica no esta haciendo lo que alguien creyo
            // haberle mandado hacer.
            || self.configuracion_sin_firmar
            || self.configuracion_no_verifica
            // Un sensor sin colector cumple su trabajo local y **no cumple la
            // promesa del producto**: que la alerta salga del equipo. Que sea
            // deliberado no lo hace menos cierto, y ocultarlo por deliberado es
            // como se despliega una flota entera sin vigilar (RPT-054 §1).
            //
            // Silenciarlo cuando la ausencia de colector sea una decision
            // declarada es cosa de la configuracion firmada, no de aqui: PA-79.
            || self.sin_colector
    }

    /// Las trece condiciones con su identificador, en el orden del contrato.
    ///
    /// RPT-058, PA-114.
    ///
    /// # Por que existe
    ///
    /// Cada consumidor escribia su propia lista a mano: `EMISIBLES` y `valor_de`
    /// en el agente, la tabla de VIS-04, el panel de diagnostico, y **el resumen
    /// por pantalla del propio agente**, que se quedo en siete de diez sin que
    /// nadie lo notara hasta verlo en una consola.
    ///
    /// Un sitio mas donde vive el contrato es un sitio mas que puede divergir. La
    /// desestructuracion de aqui es exhaustiva y **sin `..`**: un campo nuevo en
    /// `Condiciones` deja de compilar en esta funcion, y quien lo anada tiene que
    /// decidir donde va en el orden.
    ///
    /// El orden es el de [`CAMPOS_CONDICIONES`], y una prueba lo sujeta. No es
    /// estetico: es lo que permite que cualquiera de los dos sea la autoridad.
    #[must_use]
    pub const fn enumerar(&self) -> [(&'static str, bool); 13] {
        let Self {
            inventario_suprimido,
            inventario_no_verifica,
            observacion_saturada,
            captura_con_perdida,
            captura_no_disponible,
            accion_administrativa,
            salida_no_disponible,
            sin_colector,
            escucha_no_disponible,
            configuracion_sin_firmar,
            configuracion_no_verifica,
            registro_saturado,
            evidencia_en_riesgo,
        } = *self;

        [
            ("inventarioSuprimido", inventario_suprimido),
            ("inventarioNoVerifica", inventario_no_verifica),
            ("observacionSaturada", observacion_saturada),
            ("capturaConPerdida", captura_con_perdida),
            ("capturaNoDisponible", captura_no_disponible),
            ("accionAdministrativa", accion_administrativa),
            ("salidaNoDisponible", salida_no_disponible),
            ("sinColector", sin_colector),
            ("escuchaNoDisponible", escucha_no_disponible),
            ("configuracionSinFirmar", configuracion_sin_firmar),
            ("configuracionNoVerifica", configuracion_no_verifica),
            ("registroSaturado", registro_saturado),
            ("evidenciaEnRiesgo", evidencia_en_riesgo),
        ]
    }

    /// Indica si alguna condicion sugiere que **alguien toco el almacen**.
    ///
    /// Se separa de [`Self::hay_degradacion`] por la misma razon que
    /// `EstadoArranque::es_manipulacion` se separo de `exige_alerta`: quien
    /// consuma esto debe poder presentar de forma distinta «hay que reemitir el
    /// inventario» y «alguien borro el inventario».
    #[must_use]
    pub const fn hay_manipulacion(&self) -> bool {
        self.inventario_suprimido || self.inventario_no_verifica
    }
}

/// Campos de [`PeticionAlertas`].
pub const CAMPOS_PETICION_ALERTAS: [(&str, &str); 1] = [("desdeAsiento", "entero")];

/// Campos de [`SucesoAlerta`].
pub const CAMPOS_SUCESO_ALERTA: [(&str, &str); 4] = [
    ("asiento", "entero"),
    ("clase", "enumerado"),
    ("dispositivo", "texto"),
    ("detalle", "texto"),
];

/// Respuesta de `consultar-alertas`.
///
/// RPT-041, PA-74.
///
/// # Por que no es un array desnudo
///
/// Lo era, y tras la segmentacion de PA-59 se volvio una **vista parcial con
/// apariencia de exhaustividad**: quien pedia `desdeAsiento: 0` recibia las
/// alertas del segmento activo y no tenia forma de saber que habia diez mil
/// asientos archivados antes.
///
/// «No hay nada» y «esto no empieza aqui» no son lo mismo, y colapsarlos es como
/// un operador concluye que un incidente no ocurrio (RPT-006 §4).
///
/// # Por que no es un error
///
/// Pedir desde el cero es una peticion legitima y la respuesta es **correcta**:
/// las alertas devueltas existen y son exactas. Lo que faltaba no era validez,
/// era contexto. Devolver `AsientoFueraDeRango` habria dejado al cliente sin las
/// alertas vivas y adivinando desplazamientos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RespuestaAlertas {
    /// Numero de asiento mas antiguo que **sobrevive en disco**.
    ///
    /// No es «lo que este canal alcanza»: es lo que hay. Un cliente que pidio
    /// desde el cero y recibe 10001 sabe que esos diez mil existieron y estan
    /// archivados — no perdidos, no inexistentes.
    pub primer_disponible: u64,
    /// **La respuesta se corto**: hay asientos posteriores sin entregar.
    ///
    /// RPT-049, PA-96.
    ///
    /// `primer_disponible` cubria el lado antiguo —«lo de antes esta
    /// archivado»— y nadie cubria el nuevo. Quien recibia 256 sucesos no tenia
    /// forma de saber si eran todos o el principio de dos mil, salvo adivinar
    /// por el tamano del lote.
    ///
    /// Acotar la respuesta **sin decirlo** convierte un rechazo ruidoso en una
    /// lista silenciosamente incompleta, que es peor: el operador la lee como
    /// el historico entero.
    pub hay_mas: bool,
    /// Alertas que caben en esta respuesta, desde `desdeAsiento` exclusive.
    pub sucesos: Vec<SucesoAlerta>,
}

/// Campos de [`RespuestaAlertas`].
pub const CAMPOS_RESPUESTA_ALERTAS: [(&str, &str); 3] = [
    ("primerDisponible", "entero"),
    ("hayMas", "booleano"),
    ("sucesos", "lista"),
];

/// Campos de [`Condiciones`].
pub const CAMPOS_CONDICIONES: [(&str, &str); 13] = [
    ("inventarioSuprimido", "booleano"),
    ("inventarioNoVerifica", "booleano"),
    ("observacionSaturada", "booleano"),
    ("capturaConPerdida", "booleano"),
    ("capturaNoDisponible", "booleano"),
    ("accionAdministrativa", "booleano"),
    ("salidaNoDisponible", "booleano"),
    ("sinColector", "booleano"),
    ("escuchaNoDisponible", "booleano"),
    ("configuracionSinFirmar", "booleano"),
    ("configuracionNoVerifica", "booleano"),
    ("registroSaturado", "booleano"),
    ("evidenciaEnRiesgo", "booleano"),
];
