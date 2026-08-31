//! Punto de entrada del demonio Eje-Agente.
//!
//! RPT-020, PA-44.
//!
//! # Que hace y que no
//!
//! Recorre el camino completo por primera vez: **captura → observacion →
//! clasificacion → veredicto → condiciones**. Hasta ahora cada pieza estaba
//! verificada por separado y ninguna habia visto un paquete.
//!
//! Lo que **no** hace, y conviene decirlo antes de que alguien lo suponga:
//!
//! - **No carga inventario.** `arrancar` de RPT-017 exige dos claves y no existe
//!   aprovisionamiento que las entregue. El agente opera como primer arranque,
//!   que es el estado honesto: sin marcados, la clasificacion resuelve por
//!   segmento (RPT-009 §5).
//! - **No declara segmentos.** Sin manifiesto verificado, toda VLAN sale como
//!   `NoDeclarado` y ningun equipo sin marcado es contenible. La traduccion de
//!   etiqueta a declaracion ya no vive aqui —vivia, y por eso ninguna VLAN
//!   podia declararse limpia (RPT-022 §1)—; ahora la aporta
//!   [`EstadoArranque::declaracion_para`], que sin manifiesto no aporta nada.
//! - **No contiene nada.** Calcula el veredicto y lo imprime. La emision hacia un
//!   conmutador sigue bloqueada en PA-22.
//! - **No anexa a ALM-01.** Los manejadores de RPT-019 son PA-43.
//!
//! # Dos modos
//!
//! `--ciclos 1` —el valor por omision— es el recorrido de comprobacion de
//! siempre. `--ciclos 0` es el servicio continuo de RPT-034: observa, clasifica,
//! persiste **si algo cambio**, emite y atiende consultas, en ese orden y en
//! bucle.
//!
//! El modo por defecto no cambia. Convertir el agente en demonio por omision
//! habria alterado lo que hace un binario ya existente sin que nadie lo pidiera.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eje_agente::alertas::{CargaRegistro, EstadoConfiguracion, apartar, cargar_desde};
use eje_agente::ciclo::{Ciclo, Observacion, Resultado};
use eje_agente::salida::{DespachoTcp, Emisor, INTERVALO_LATIDO_MS, Latido, colector_declarado};
use eje_agente::servicio::{Escucha, Manejadores};
use eje_agente::{ConfiguracionAgente, VERSION};
use eje_captura::transporte::extraer;
use eje_captura::{DireccionEnlace, ErrorCaptura, FuentePasiva, abrir};
use eje_ipc::mensajes::EstadoAgente;
use guardian_cc::PerfilSegmento;
use guardian_cc::arranque::{
    ErrorArranque, EstadoArranque, RutasAlmacen, aceptar_configuracion, arrancar_con_almacen,
    cargar_centinela,
};
use guardian_cc::clave::analizar as analizar_clave;
use guardian_cc::inventario::DominioClave;
use guardian_cc::observacion::Protocolo;
use guardian_cc::proveedores::ProveedorHuella;

/// Plazo de espera por trama.
const PLAZO: Duration = Duration::from_millis(500);

/// Tramas a observar antes de resumir, si no se indica otra cosa.
const TRAMAS_POR_DEFECTO: u64 = 200;

/// Directorio de almacen por defecto, relativo al directorio de trabajo.
const ALMACEN_POR_DEFECTO: &str = "datos-eje";

/// Plazo para conectar y escribir al colector de syslog.
///
/// Corto a proposito: un colector que no responde no debe detener la
/// observacion. Lo que no sale queda en ALM-01 y la condicion lo dice.
const PLAZO_SYSLOG: Duration = Duration::from_secs(3);

/// Fallos del arranque del agente.
#[derive(Debug, thiserror::Error)]
enum ErrorAgente {
    /// Faltan argumentos o son incorrectos.
    // RPT-071, PA-122. El texto sale de `OPCIONES` y no de una cadena escrita a
    // mano: `--directorio-socket` existio un dia entero sin aparecer aqui.
    #[error("{}", uso())]
    Uso,

    /// Un argumento pretende dictar lo que la configuracion firmada ya dicta.
    ///
    /// RPT-074 §10, PA-79. **Es un error de arranque y no un aviso.** Un aviso
    /// se lee una vez en el diario y el sensor sigue corriendo con los
    /// parametros de quien controle la unidad, que es exactamente lo que la
    /// firma existe para impedir: la firma no vale nada si `--interfaz` puede
    /// dejarla sin efecto.
    #[error(
        "con configuracion firmada, '{0}' no se pasa por la linea de ordenes.\n\
         Los parametros de este sensor salen de {1}, que va firmado.\n\
         Cambialos ahi y reemitelo con: eje-manifiesto configurar"
    )]
    ArgumentoDictado(&'static str, &'static str),

    /// La captura no pudo abrirse.
    #[error(transparent)]
    Captura(#[from] ErrorCaptura),

    /// El almacen local no se pudo leer.
    #[error(transparent)]
    Arranque(#[from] ErrorArranque),

    /// El registro de evidencia no se pudo leer.
    ///
    /// **Solo llega aqui un fallo del sistema de ficheros**, no un registro
    /// danado: eso lo resuelve [`CargaRegistro`] sin impedir el arranque. Si el
    /// disco no responde, en cambio, el agente no puede prometer que anexara
    /// nada, y arrancar fingiendo que si seria peor que no arrancar.
    #[error(transparent)]
    Evidencia(#[from] guardian_cc::disco::ErrorDisco),
}

/// Lo que la linea de ordenes **pidio**.
///
/// RPT-074 §10, PA-79. No es lo que el agente usa: eso es [`Efectivas`], y desde
/// el paso 4b las dos cosas pueden no coincidir. Separarlas no es ceremonia —
/// mientras fueron la misma estructura, «se paso `--interfaz`» y «la interfaz
/// vale eth0» eran indistinguibles, y sin esa distincion no se puede rechazar un
/// argumento que dicta lo que la firma ya dicta.
struct Argumentos {
    /// Que banderas se teclearon, con independencia de su valor.
    ///
    /// Se llena dentro de la misma puerta que valida las banderas, de modo que
    /// una opcion nueva entra aqui por construccion. Los elementos son los
    /// `&'static str` de [`OPCIONES`], no copias: comparar por puntero o por
    /// texto da lo mismo porque no hay dos fuentes.
    dadas: Vec<&'static str>,
    /// `None` es «no se dio». Sin configuracion firmada eso es un error de uso;
    /// con configuracion firmada es lo normal.
    interfaz: Option<String>,
    tramas: u64,
    perfil: PerfilSegmento,
    almacen: PathBuf,
    /// Colector de syslog, `host:puerto`. Sin el, la alerta no sale del equipo.
    syslog: Option<String>,
    /// Ciclos a ejecutar. `0` es servicio continuo (PA-67).
    ///
    /// Por defecto **uno**: el recorrido de comprobacion que existia antes sigue
    /// siendo el comportamiento de partida. Convertir el agente en demonio por
    /// omision habria cambiado lo que hace un binario ya existente sin que nadie
    /// lo pidiera.
    ciclos: u64,
    /// Grupo (numerico) autorizado a consultar por el socket. PA-82.
    ///
    /// Sin el, el socket queda en `0600` y solo el usuario del agente puede
    /// conectarse — lo que obliga a la consola a correr con `sudo` cuando el
    /// agente captura tramas (RPT-046 §11).
    ///
    /// Es numerico porque resolver un nombre exige `getgrnam`, fuera de la
    /// biblioteca estandar. El empaquetado conoce el grupo que crea. PA-84.
    grupo_ipc: Option<u32>,
    /// Cada cuanto late el sensor, en milisegundos. RPT-057, PA-105.
    ///
    /// El colector calcula la ausencia con el intervalo que el propio latido
    /// declara, asi que alargarlo alarga la ventana de silencio que la sala
    /// vigila. Desde RPT-077 lo dicta la configuracion firmada y esta bandera
    /// **solo rige sin ella**: pasarla teniendo configuracion firmada impide
    /// arrancar. Sigue existiendo para ejercitar la deteccion de ausencia en
    /// laboratorio sin esperar minutos.
    intervalo_latido: i64,
    /// Nombre con el que este sensor se identifica ante el colector. RPT-058.
    ///
    /// Es el campo `HOSTNAME` de RFC 5424, y es **la identidad del sensor en la
    /// sala**: por el se correlacionan los sellos de RPT-038 y por el se detecta
    /// la ausencia de latidos (PA-105).
    ///
    /// `None` es «no se dio»: la identidad cae entonces al nombre de la maquina,
    /// y esa caida ocurre al resolver y no al leer, para que «no se dio» siga
    /// siendo distinguible de «se dio justo el nombre de la maquina».
    nombre: Option<String>,
    /// Directorio volatil donde se abre el socket. RPT-067, PA-120.
    ///
    /// `None` significa el de fabrica, `/run/eje-latam`, que es el que `systemd`
    /// crea con `RuntimeDirectory=`. Se puede mover el directorio y **no** el
    /// nombre del fichero: si la ruta completa fuera configurable, nada
    /// impediria devolverla al directorio de evidencia y deshacer la separacion
    /// sin que ninguna comprobacion se enterase.
    ///
    /// Existe porque crear `/run/eje-latam` exige root, y obligar a `sudo` para
    /// levantar la consola de diagnostico haria que nadie la levantara.
    directorio_socket: Option<PathBuf>,
}

/// Los parametros con los que el agente **corre de verdad**.
///
/// RPT-074 §10, PA-79. Se construyen resolviendo [`Argumentos`] contra la
/// configuracion firmada, y a partir de aqui nadie vuelve a mirar la linea de
/// ordenes: si quedara alguna lectura suelta de `Argumentos` mas abajo, seria
/// justo el hueco por el que se recupera el mando.
///
/// # Lo que falta aqui a proposito
///
/// **No lleva el almacen ni el directorio del socket.** Esos dos se fijan antes,
/// en [`rutas_de_instalacion`], porque la clave que verifica la configuracion
/// firmada vive dentro del almacen (RPT-077). Que no esten en esta estructura no
/// es un olvido: es lo que hace **imposible** que la resolucion los mueva. Un
/// comentario pidiendo que nadie los toque se habria roto el primer dia.
struct Efectivas {
    /// `None` significa que **no hay nada que vigilar**, y ocurre en un solo
    /// caso: [`Configuracion::NoVerifica`]. No se disfraza de interfaz
    /// inexistente porque `capturaNoDisponible` ya dice la verdad de lo que pasa.
    interfaz: Option<String>,
    tramas: u64,
    perfil: PerfilSegmento,
    syslog: Option<String>,
    ciclos: u64,
    grupo_ipc: Option<u32>,
    intervalo_latido: i64,
    nombre: String,
}

/// La configuracion firmada que este agente encontro, si encontro alguna.
///
/// RPT-074 §10, PA-79. Tres estados y no dos (RPT-006 §4): que no haya fichero y
/// que lo haya y no valga son cosas distintas, y la diferencia se pierde en
/// cuanto alguien las resuelve con un `bool`.
///
/// El motivo viaja **dentro** del estado que lo tiene, en lugar de al lado en un
/// `Option<String>` que podria estar lleno cuando no toca o vacio cuando si.
enum Configuracion {
    /// Verificada y dirigida a esta maquina.
    ///
    /// En caja: `Valores` tiene diez campos y las otras dos variantes casi nada,
    /// asi que sin caja toda la enumeracion ocuparia lo que ocupa la mayor.
    Firmada(Box<guardian_cc::configuracion::Valores>),
    /// No hay fichero, o no se puede leer. Son la misma cosa para este agente:
    /// no tiene configuracion firmada que obedecer, y el remedio es el mismo.
    Ausente,
    /// Hay fichero y este agente no lo acepta, por el motivo que se adjunta.
    NoVerifica(String),
}

impl Configuracion {
    /// La condicion que este estado enciende en el latido.
    const fn estado(&self) -> EstadoConfiguracion {
        match self {
            Self::Firmada(_) => EstadoConfiguracion::Firmada,
            Self::Ausente => EstadoConfiguracion::Ausente,
            Self::NoVerifica(_) => EstadoConfiguracion::NoVerifica,
        }
    }
}

/// Nombre de esta maquina, o una declaracion de que no se pudo averiguar.
///
/// # Por que no se cae hacia algo plausible
///
/// Hasta RPT-058 se usaba **el nombre de la interfaz**, y el resultado fue que en
/// la sala el sensor se llamaba `lo`. Dos sensores de dos hospitales distintos
/// con la interfaz `eth0` serian el mismo sensor para el vigia, y **el latido de
/// uno taparia la muerte del otro**: exactamente el fallo que PA-104 existe para
/// impedir.
///
/// Asi que si el nombre no se puede leer no se sustituye por otra cosa que
/// parezca un nombre. Se declara que no se sabe, con un valor que nadie confunde
/// con una maquina, y el operador lo ve en la primera linea del arranque.
fn nombre_de_maquina() -> String {
    // `/proc/sys/kernel/hostname` es la fuente del nucleo en Linux, que es donde
    // corre el agente porque la captura solo existe ahi. Se prueba `/etc/hostname`
    // despues por si el `/proc` no esta montado en un contenedor recortado.
    for ruta in ["/proc/sys/kernel/hostname", "/etc/hostname"] {
        if let Ok(leido) = std::fs::read_to_string(ruta) {
            let limpio = leido.trim();
            if !limpio.is_empty() {
                return limpio.to_owned();
            }
        }
    }

    "SIN-NOMBRE-DE-MAQUINA".to_owned()
}

/// Describe un estado de arranque para el operador.
///
/// Se escribe aqui y no en `guardian-cc` a proposito: el texto que ve una
/// persona es presentacion, y meterlo en la biblioteca invitaria a que alguna
/// decision se tomara comparando cadenas.
const fn describir(estado: &EstadoArranque) -> &'static str {
    match estado {
        EstadoArranque::Operativo(_) => "operativo, inventario verificado",
        EstadoArranque::PrimerArranque => "primer arranque, sin inventario todavia",
        EstadoArranque::SinClaveAprovisionada => "SIN CLAVE aprovisionada; nada se puede verificar",
        EstadoArranque::FormatoObsoleto { .. } => {
            "formato anterior; hay que reemitir el inventario"
        }
        EstadoArranque::Supresion { .. } => "SUPRESION: habia inventario y ya no esta",
        EstadoArranque::NoVerifica { .. } => "el inventario NO VERIFICA",
    }
}

/// Una opcion de la linea de ordenes.
///
/// # Por que es una tabla y no una cadena de uso escrita a mano
///
/// RPT-071, PA-122. `--directorio-socket` se anadio al analizador y **no** a la
/// linea de uso, y estuvo un dia entero siendo una opcion que nadie podia
/// descubrir. Se vio en la prueba de humo de RPT-069, no revisando el codigo.
///
/// Es el cuarto indice escrito a mano de la semana: el tablero (PA-108), las
/// pruebas (PA-73), el manual de comandos (PA-119) y este. De aqui sale la linea
/// de uso, y contra esto se contrasta lo que el analizador acepta.
struct Opcion {
    /// Como se teclea.
    bandera: &'static str,
    /// Que valor toma, para la linea de uso.
    valor: &'static str,
    /// Un valor valido, para que la prueba pueda ejercitarla.
    ///
    /// Existe **solo en compilaciones de prueba**. En produccion nadie lo lee, y
    /// el compilador lo dijo: `field 'ejemplo' is never read`. Las dos salidas
    /// faciles eran peores que el aviso —un `#[allow(dead_code)]` apaga un
    /// instrumento que decia la verdad, y una tabla de ejemplos dentro del modulo
    /// de pruebas seria un quinto indice escrito a mano, justo dentro de la
    /// barrera que existe para cazar esos—. Se retira del binario en lugar de
    /// callar el aviso.
    #[cfg(test)]
    ejemplo: &'static str,
    /// Si el agente se niega a arrancar sin ella **cuando no hay configuracion
    /// firmada**. Con configuracion firmada no hace falta ninguna.
    obligatoria: bool,
    /// Si una configuracion firmada valida dicta este valor. RPT-074 §10, PA-79.
    ///
    /// Las dictadas **no se aceptan** por la linea de ordenes mientras exista
    /// configuracion firmada: pasarlas es un error de arranque, no un aviso.
    ///
    /// Se rechaza incluso cuando el argumento coincide con lo firmado. Comparar
    /// obligaria a decidir que significa «igual» para cada tipo —una ruta con
    /// barra final, un colector con mayusculas, un intervalo escrito distinto— y
    /// cada una de esas decisiones es un sitio donde colar un valor que pasa por
    /// igual sin serlo. La regla que no admite grados es mas facil de sostener:
    /// **con configuracion firmada, la linea de ordenes no dicta nada.**
    dictada: bool,
}

/// Todas las opciones que `eje-agente` acepta. Fuente unica.
const OPCIONES: &[Opcion] = &[
    Opcion {
        bandera: "--interfaz",
        valor: "<nombre>",
        #[cfg(test)]
        ejemplo: "lo",
        obligatoria: true,
        dictada: true,
    },
    Opcion {
        bandera: "--tramas",
        valor: "<n>",
        #[cfg(test)]
        ejemplo: "10",
        obligatoria: false,
        // Cuantas tramas se miran por vuelta no describe al sensor: describe al
        // que lo esta mirando. No viaja firmado y por eso sigue admitiendose.
        dictada: false,
    },
    Opcion {
        bandera: "--perfil",
        valor: "corporativo|ot",
        #[cfg(test)]
        ejemplo: "ot",
        obligatoria: false,
        dictada: true,
    },
    Opcion {
        bandera: "--almacen",
        valor: "<ruta>",
        #[cfg(test)]
        ejemplo: "/var/lib/eje-latam",
        obligatoria: false,
        // NO la dicta la configuracion firmada, y no por comodidad: la clave con
        // la que esa configuracion se verifica es `<almacen>/clave-cliente.pub`.
        // Una configuracion que moviera el almacen estaria eligiendo donde se
        // busca la clave que decide si creerla (RPT-077). Donde guarda sus
        // ficheros esta maquina lo decide quien la instala, en la unidad.
        dictada: false,
    },
    Opcion {
        bandera: "--directorio-socket",
        valor: "<ruta>",
        #[cfg(test)]
        ejemplo: "/run/eje-latam",
        obligatoria: false,
        // Por lo mismo: `RuntimeDirectory=` de la unidad crea este directorio, y
        // firmar otro produciria un sensor que abre el socket donde systemd no
        // le ha dado permiso de escritura (RPT-067).
        dictada: false,
    },
    Opcion {
        bandera: "--syslog",
        valor: "<host:puerto>",
        #[cfg(test)]
        ejemplo: "127.0.0.1:5514",
        obligatoria: false,
        dictada: true,
    },
    Opcion {
        bandera: "--ciclos",
        valor: "<n|0=continuo>",
        #[cfg(test)]
        ejemplo: "0",
        obligatoria: false,
        // Es la diferencia entre el servicio y un recorrido de comprobacion a
        // mano, y la unidad de systemd la pasa. No es un parametro del sensor.
        dictada: false,
    },
    Opcion {
        bandera: "--grupo-ipc",
        valor: "<gid>",
        #[cfg(test)]
        ejemplo: "1000",
        obligatoria: false,
        dictada: true,
    },
    Opcion {
        bandera: "--intervalo-latido",
        valor: "<ms>",
        #[cfg(test)]
        ejemplo: "10000",
        obligatoria: false,
        dictada: true,
    },
    Opcion {
        bandera: "--nombre",
        valor: "<maquina>",
        #[cfg(test)]
        ejemplo: "sensor-planta-3",
        obligatoria: false,
        dictada: true,
    },
];

/// La linea de uso, derivada de [`OPCIONES`].
fn uso() -> String {
    let mut linea = String::from("uso: eje-agente");
    for opcion in OPCIONES {
        let par = format!("{} {}", opcion.bandera, opcion.valor);
        if opcion.obligatoria {
            linea.push_str(&format!(" {par}"));
        } else {
            linea.push_str(&format!(" [{par}]"));
        }
    }

    // La linea de arriba describe el arranque SIN aprovisionar, que es el unico
    // en el que la linea de ordenes manda. Decirlo aqui y no en un manual: quien
    // lee esto acaba de teclear algo que no funciono.
    linea.push_str("\n\nCon ");
    linea.push_str(guardian_cc::configuracion::RUTA_CONFIGURACION);
    linea.push_str(" presente y valido,\nlos parametros del sensor salen de ahi (");
    let dictadas: Vec<&str> = OPCIONES
        .iter()
        .filter(|opcion| opcion.dictada)
        .map(|opcion| opcion.bandera)
        .collect();
    linea.push_str(&dictadas.join(" "));
    linea.push_str(
        ")\ny pasarlos por aqui es un error de arranque. Emitela con: eje-manifiesto configurar",
    );
    linea
}

fn leer_opciones() -> Result<Argumentos, ErrorAgente> {
    leer_opciones_de(&std::env::args().skip(1).collect::<Vec<String>>())
}

/// Igual, sobre argumentos dados. Existe para que la prueba de PA-122 pueda
/// ejercitar cada opcion sin tocar el entorno del proceso.
///
/// **Aqui ya no se decide nada.** Desde RPT-074 §10 esto solo traduce texto a
/// valores y anota que se pidio; quien manda lo decide [`resolver`], que es
/// donde vive la configuracion firmada. Un valor por omision aplicado aqui
/// —como estaba `nombre`— haria indistinguible «no se dio» de «se dio esto», y
/// sin esa distincion no se puede rechazar lo que la firma ya dicta.
fn leer_opciones_de(argumentos: &[String]) -> Result<Argumentos, ErrorAgente> {
    let mut dadas: Vec<&'static str> = Vec::new();
    let mut interfaz = None;
    let mut tramas = TRAMAS_POR_DEFECTO;
    let mut perfil = PerfilSegmento::Corporativo;
    let mut almacen = PathBuf::from(ALMACEN_POR_DEFECTO);
    let mut syslog = None;
    let mut ciclos = 1u64;
    let mut grupo_ipc = None;
    let mut intervalo_latido = INTERVALO_LATIDO_MS;
    let mut nombre = None;
    let mut directorio_socket = None;

    let mut indice = 0;

    while indice < argumentos.len() {
        let clave = argumentos[indice].as_str();
        let Some(valor) = argumentos.get(indice + 1) else {
            return Err(ErrorAgente::Uso);
        };

        // RPT-071, PA-122. La puerta esta ANTES del `match`: asi no se puede
        // aceptar una bandera que la linea de uso no anuncie. La direccion
        // contraria —anunciar una que el analizador ignore— la cubre una prueba,
        // porque un `match` no se puede enumerar sin leer el fuente, y este
        // proyecto ya aprendio lo que cuesta leer fuente sin lexer.
        let Some(opcion) = OPCIONES.iter().find(|opcion| opcion.bandera == clave) else {
            return Err(ErrorAgente::Uso);
        };

        // Se anota DENTRO de la puerta, con el `&'static str` de la tabla. Una
        // opcion nueva queda registrada por construccion, y no hay una segunda
        // lista de banderas que pueda quedarse corta — que es el defecto que ya
        // aparecio cinco veces esta semana.
        dadas.push(opcion.bandera);

        match clave {
            "--interfaz" => interfaz = Some(valor.clone()),
            "--tramas" => tramas = valor.parse().map_err(|_| ErrorAgente::Uso)?,
            "--perfil" => {
                perfil = match valor.as_str() {
                    "corporativo" => PerfilSegmento::Corporativo,
                    "ot" => PerfilSegmento::Ot,
                    _ => return Err(ErrorAgente::Uso),
                }
            }
            "--almacen" => almacen = PathBuf::from(valor),
            // Solo el directorio. El nombre del socket no es configurable
            // (RPT-067, PA-120).
            "--directorio-socket" => directorio_socket = Some(PathBuf::from(valor)),
            // Una cadena vacia NO es un destino. `systemd` sustituye
            // `${VARIABLE}` como un argumento aunque este vacia, asi que sin
            // esto el agente tomaba «no hay colector» por «el colector no
            // responde» — las dos cosas que RPT-055 §3 separo. Ver
            // `colector_declarado` (RPT-064, PA-118).
            "--syslog" => syslog = colector_declarado(valor).map(str::to_owned),
            "--ciclos" => ciclos = valor.parse().map_err(|_| ErrorAgente::Uso)?,
            "--grupo-ipc" => grupo_ipc = Some(valor.parse().map_err(|_| ErrorAgente::Uso)?),
            // Provisional. El intervalo viaja dentro del latido para que el
            // colector no lo suponga, asi que quien controle el arranque puede
            // alargar la ventana de silencio que la sala vigila. Sale a
            // configuracion firmada en PA-79.
            "--nombre" => nombre = Some(valor.clone()),
            "--intervalo-latido" => {
                intervalo_latido = valor.parse().map_err(|_| ErrorAgente::Uso)?;
            }
            _ => return Err(ErrorAgente::Uso),
        }

        indice += 2;
    }

    Ok(Argumentos {
        dadas,
        interfaz,
        tramas,
        perfil,
        almacen,
        syslog,
        ciclos,
        grupo_ipc,
        intervalo_latido,
        nombre,
        directorio_socket,
    })
}

/// Resuelve quien manda, y con que.
///
/// RPT-074 §10 paso 4b, PA-79. Es el punto entero del paso: hasta aqui el agente
/// **leia** la configuracion firmada y la **declaraba**, pero corria con lo que
/// le dijera la linea de ordenes. Un sensor que anuncia «configuracion firmada»
/// mientras vigila la interfaz que alguien le puso en el `ExecStart` es
/// precisamente la mentira contra la que se escribio RPT-074.
///
/// # Los tres caminos
///
/// - **Firmada** — manda ella, entera. Cualquier bandera marcada `dictada` en
///   [`OPCIONES`] aborta el arranque, coincida o no con lo firmado.
/// - **Ausente** — manda la linea de ordenes, `--interfaz` vuelve a hacer falta
///   y `configuracionSinFirmar` lo cuenta en cada latido. Es como corre hoy toda
///   la flota, y por eso el paso no rompe ningun despliegue existente.
/// - **NoVerifica** — **no manda nadie.** Ni la firma, que no verifica, ni la
///   linea de ordenes: a quien pudo tocar el fichero le bastaria romperlo para
///   recuperar el mando por argumentos. El agente arranca sin parametros —no
///   captura, no emite— y lo declara. Arrancar y no morirse es deliberado: bajo
///   `Restart=always` un proceso que sale con error es un bucle de reinicios, y
///   ademas `configuracionNoVerifica` no se encenderia nunca (RPT-077 §5).
///
/// # Errores
///
/// [`ErrorAgente::ArgumentoDictado`] si se paso un parametro que la firma dicta,
/// y [`ErrorAgente::Uso`] si no hay configuracion y falta algo obligatorio.
fn resolver(
    argumentos: Argumentos,
    configuracion: &Configuracion,
) -> Result<Efectivas, ErrorAgente> {
    match configuracion {
        Configuracion::Firmada(valores) => {
            for bandera in &argumentos.dadas {
                let dictada = OPCIONES
                    .iter()
                    .any(|opcion| opcion.bandera == *bandera && opcion.dictada);
                if dictada {
                    return Err(ErrorAgente::ArgumentoDictado(
                        bandera,
                        guardian_cc::configuracion::RUTA_CONFIGURACION,
                    ));
                }
            }

            Ok(Efectivas {
                interfaz: Some(valores.interfaz.clone()),
                // De la linea de ordenes: no viajan firmados porque no describen
                // al sensor. Ver los comentarios de sus filas en `OPCIONES`.
                tramas: argumentos.tramas,
                ciclos: argumentos.ciclos,
                perfil: valores.perfil,
                // Vacio es un destino legitimo y significa «ninguno» (RPT-054 §1,
                // RPT-064). Se pasa por el mismo filtro que la linea de ordenes
                // para que las dos vias signifiquen lo mismo.
                syslog: colector_declarado(&valores.colector).map(str::to_owned),
                grupo_ipc: valores.grupo_ipc,
                // Un intervalo que no cabe en `i64` no se recorta a algo
                // plausible: se lleva al maximo, con lo que el sensor latiria
                // practicamente nunca y `sinColector` no lo taparia. Es un valor
                // absurdo declarado como absurdo, no uno inventado.
                intervalo_latido: i64::try_from(valores.intervalo_latido_ms).unwrap_or(i64::MAX),
                nombre: valores.nombre.clone(),
            })
        }

        Configuracion::Ausente => Ok(Efectivas {
            // RPT-080, PA-133. Sin interfaz, la respuesta depende de quién
            // pregunta, y es la misma distinción que ya hace `--ciclos`:
            //
            // - **El servicio arranca y lo declara.** Morir aquí es un bucle de
            //   reinicios bajo `Restart=always`, no una avería visible. Se
            //   observó ocurriendo 350 veces seguidas sin que nada fuera del
            //   diario local se enterase (RPT-079 §11).
            // - **A mano se explica el uso**, porque hay una persona delante que
            //   acaba de teclear algo incompleto y quiere saber qué falta.
            //
            // Es el argumento de RPT-077 §5 —arrancar y declarar en lugar de
            // morir— aplicado al caso que aquel reporte dejó fuera por
            // descuido: allí se razonó para la firma **rota** y no para la
            // firma **ausente**.
            interfaz: match argumentos.interfaz {
                Some(interfaz) => Some(interfaz),
                None if es_servicio(argumentos.ciclos) => None,
                None => return Err(ErrorAgente::Uso),
            },
            tramas: argumentos.tramas,
            perfil: argumentos.perfil,
            syslog: argumentos.syslog,
            ciclos: argumentos.ciclos,
            grupo_ipc: argumentos.grupo_ipc,
            intervalo_latido: argumentos.intervalo_latido,
            // La caida al nombre de la maquina ocurre aqui y no al analizar, para
            // que `--nombre` siga siendo distinguible de su ausencia.
            nombre: argumentos.nombre.unwrap_or_else(nombre_de_maquina),
        }),

        Configuracion::NoVerifica(_) => Ok(Efectivas {
            // Nada de lo que dicta la firma, y nada de lo que dice la linea de
            // ordenes. Sin interfaz no hay captura, y sin colector no sale nada:
            // las dos cosas las cuentan sus propias condiciones sin que este
            // modo tenga que inventarse ninguna.
            interfaz: None,
            syslog: None,
            perfil: PerfilSegmento::Corporativo,
            grupo_ipc: None,
            intervalo_latido: INTERVALO_LATIDO_MS,
            // El almacen y el socket no aparecen aqui —ni en las otras dos
            // ramas— porque los fija `rutas_de_instalacion` antes de resolver
            // nada. Hace falta ademas que en este modo sigan siendo los de
            // siempre: un agente que declarara su averia escribiendo en otro
            // directorio y abriendo otro socket seria un agente al que la
            // consola de diagnostico no encuentra justo cuando hace falta.
            //
            // Sin la identidad firmada, la de la maquina. Es la unica que no
            // depende de nadie que pueda estar equivocandose.
            nombre: nombre_de_maquina(),
            // `--ciclos` y `--tramas` no dictan nada del sensor: dictan cuanto
            // dura esta ejecucion. Si se los quitara, `--ciclos 0` de la unidad
            // dejaria de funcionar y el modo se convertiria en un proceso que
            // arranca, da una vuelta y muere — que bajo `Restart=always` es un
            // bucle de reinicios, no un sensor que declara nada (RPT-072).
            tramas: argumentos.tramas,
            ciclos: argumentos.ciclos,
        }),
    }
}

/// Donde guarda sus ficheros **esta maquina**.
///
/// RPT-077, PA-79. Se decide **antes** de leer la configuracion firmada, y esa
/// precedencia es el punto entero: la clave con la que esa configuracion se
/// verifica es `<almacen>/clave-cliente.pub`, asi que si el almacen saliera de la
/// propia configuracion, la configuracion estaria eligiendo donde se busca la
/// clave que decide si creerla.
///
/// Dos directorios y no uno: lo que sobrevive al reinicio y lo que no deben vivir
/// separados (RPT-067, PA-120).
fn rutas_de_instalacion(argumentos: &Argumentos) -> RutasAlmacen {
    argumentos.directorio_socket.clone().map_or_else(
        || RutasAlmacen::nuevo(argumentos.almacen.clone()),
        |volatil| RutasAlmacen::con_directorio_socket(argumentos.almacen.clone(), volatil),
    )
}

/// Protocolo que un puerto **sugiere**.
///
/// # Esto no es huella
///
/// Un puerto no prueba un protocolo. Modbus movido al 10502 se escapa, y
/// cualquiera puede abrir el 502 y hablar otra cosa.
///
/// Se admite porque alimenta una fuente **inferida**, y por RPT-009 §3 esas solo
/// pueden sugerir criticidad, nunca descartarla: un falso negativo deja el
/// dispositivo donde ya estaba, y un falso positivo lleva a ambiguedad y a un
/// humano. Ninguna de las dos direcciones concede permiso.
///
/// La taxonomia de verdad es trabajo de dominio (RPT-018 §8.1) y esta tabla
/// debera moverse a donde viva esa decision.
const fn protocolo_de(puerto: u16) -> Option<Protocolo> {
    match puerto {
        502 => Some(Protocolo::Modbus),
        20_000 => Some(Protocolo::Dnp3),
        2_575 => Some(Protocolo::Hl7),
        47_808 => Some(Protocolo::Bacnet),
        _ => None,
    }
}

/// Instante actual en segundos desde la epoca.
///
/// Un reloj anterior a la epoca devuelve `0`, con lo que toda vigencia se lee
/// como caducada. Es la direccion segura y la misma que aplican
/// `MarcadoVerificado::vigente_en` y `DeclaracionVlan::vigente_en`.
fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |transcurrido| transcurrido.as_secs())
}

/// Punto de entrada.
///
/// # Por que no devuelve `Result`
///
/// Devolverlo es lo comodo, y Rust imprime el error con **`Debug`**, no con
/// `Display`. El resultado era `Error: Uso` — el nombre de la variante— en lugar
/// de la linea de uso que `#[error(...)]` ya tenia escrita.
///
/// Costo dos rondas de diagnostico (RPT-046 §11.2, PA-85): un binario viejo
/// rechazaba `--grupo-ipc` y decia `Error: Uso`. Con el uso impreso se habria
/// visto de inmediato que la opcion no figuraba.
fn main() {
    if let Err(error) = ejecutar() {
        // `{error}` y no `{error:?}`: lo que se escribio para una persona debe
        // llegar a esa persona.
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn ejecutar() -> Result<(), ErrorAgente> {
    let argumentos = leer_opciones()?;

    // El orden de estas lineas es la correccion entera de los pasos 4b y 5
    // (RPT-074 §10, RPT-077, RPT-078):
    //
    //   1. la instalacion —donde estan los ficheros de esta maquina— se fija
    //      desde la linea de ordenes, porque ahi vive la clave;
    //   2. se leen las dos marcas de agua, que viven en el mismo fichero;
    //   3. con la clave y la marca se lee, se verifica y se **fecha** la
    //      configuracion firmada;
    //   4. si es buena, la marca avanza en disco ANTES de obedecerla;
    //   5. la configuracion decide que parametros rigen, y si alguno venia
    //      tambien por la linea de ordenes el agente **no arranca**;
    //   6. de aqui en adelante nadie vuelve a mirar `argumentos`.
    //
    // Invertir 1 y 2 es imposible —haria falta la clave para saber donde esta la
    // clave— y ese circulo es justo lo que saco `almacen` de la firma.
    let rutas = rutas_de_instalacion(&argumentos);
    let centinelas = cargar_centinela(&rutas)?;
    let configuracion = leer_configuracion(&rutas, &nombre_de_maquina(), centinelas.configuracion);
    let configuracion = anotar_configuracion(&rutas, configuracion);
    let opciones = resolver(argumentos, &configuracion)?;
    let ajustes = ConfiguracionAgente::para_segmento(opciones.perfil);

    println!("Eje-Agente {VERSION}");
    println!(
        "Interfaz           : {}",
        opciones
            .interfaz
            .as_deref()
            .unwrap_or("NINGUNA; este sensor no esta vigilando nada")
    );
    println!("Identidad en la sala: {}", opciones.nombre);
    if opciones.nombre == "SIN-NOMBRE-DE-MAQUINA" {
        println!("  !! No se pudo leer el nombre de esta maquina.");
        println!("     En la sala este sensor no se distinguira de otro igual.");
        println!("     Dale un nombre con --nombre antes de desplegarlo.");
    }
    println!("Perfil de segmento : {:?}", ajustes.perfil);
    println!("Capa B autorizada  : {}", ajustes.red.capa_b_autorizada);

    // RPT-074 §10, PA-79. Quien mando, y aqui arriba a proposito: las tres lineas
    // anteriores son parametros, y esta dice de donde salieron. Ya no es una nota
    // informativa —desde el paso 4b describe con que esta corriendo de verdad el
    // proceso—, asi que leerla despues de haberse creido las otras seria tarde.
    match &configuracion {
        Configuracion::Firmada(valores) => {
            println!(
                "Configuracion      : FIRMADA y verificada (secuencia {})",
                valores.secuencia
            );
            println!("     Los parametros de arriba salen de ahi. La linea de ordenes");
            println!("     no puede cambiarlos: si lo intenta, el agente no arranca.");
        }
        Configuracion::Ausente if opciones.interfaz.is_none() => {
            // RPT-080, PA-133. Un sensor instalado y sin aprovisionar. Arranca
            // —morir seria un bucle de reinicios— y dice con todas las letras
            // que no esta haciendo su trabajo. Las condiciones lo cuentan por el
            // socket; esto es la mitad que ve quien esta delante de la maquina.
            println!("Configuracion      : SIN FIRMAR, y no hay ninguna interfaz que vigilar");
            println!("  !! ESTE SENSOR NO ESTA VIGILANDO NADA.");
            println!("     Arranca, atiende consultas y lo declara. No observa.");
            println!("     Emitele configuracion firmada: eje-manifiesto configurar");
            println!(
                "     El campo 'maquina' tiene que ser: {}",
                nombre_de_maquina()
            );
        }
        Configuracion::Ausente => {
            println!("Configuracion      : SIN FIRMAR");
            println!("     Los parametros salen de la linea de ordenes, asi que quien");
            println!("     controle el arranque puede alargar la ventana de silencio");
            println!("     que la sala vigila. Emitela con: eje-manifiesto configurar");
        }
        Configuracion::NoVerifica(motivo) => {
            println!("Configuracion      : NO VERIFICA");
            println!("     {motivo}");
            println!("  !! Hay un fichero de configuracion y este agente NO lo acepta.");
            println!("     No se cae a la linea de ordenes: a quien pudo tocar el fichero");
            println!("     le bastaria romperlo para recuperar el mando por argumentos.");
            println!("     Este agente no vigila nada y no emite nada. Solo lo declara.");
        }
    }
    let estado_configuracion = configuracion.estado();
    println!();

    // PA-49. El agente lee su propio estado del almacen: claves, centinela,
    // revocaciones e inventario. Hasta aqui construia `PrimerArranque` a mano
    // porque `arrancar` exigia dos claves que nadie le daba.
    let (estado, _centinela) = arrancar_con_almacen(&rutas)?;

    // PA-58. El registro se carga del disco ANTES de observar nada, para que las
    // alertas de esta ejecucion continuen la serie en lugar de reiniciarla.
    let carga = cargar_desde(&rutas.evidencia())?;
    let aviso_registro = describir_carga(&carga);

    if let CargaRegistro::ViolacionDetectada { .. } = carga {
        // Se aparta, no se borra. Si el renombrado falla se avisa y se sigue
        // observando: quedarse sin vigilancia por un fallo de disco seria un
        // dano cierto para evitar uno ya ocurrido.
        match apartar(&rutas.evidencia(), instante_utc()) {
            Ok(destino) => println!(
                "  !! El registro anterior se aparto en {}",
                destino.display()
            ),
            Err(error) => println!("  !! No se pudo apartar el registro danado: {error}"),
        }
    }

    let registro = carga.registro();

    // PA-42. Sin `--syslog` no hay emisor y la alerta no sale del equipo. Se
    // dice por pantalla en lugar de suponerse: un agente que no emite y no lo
    // anuncia parece uno que emite y nadie escucha.
    //
    // Y sin interfaz tampoco hay emisor: RFC 5424 pide un `APP-NAME` que diga de
    // que sensor y de que boca sale la alerta, y sin captura no hay boca. Antes
    // se resolvia con `zip` de nada porque la interfaz siempre existia; desde
    // que puede faltar, las dos cosas tienen que estar.
    let emisor = opciones
        .syslog
        .as_ref()
        .zip(opciones.interfaz.as_ref())
        .map(|(destino, interfaz)| {
            // La identidad es la MAQUINA, no la interfaz. Ver `nombre_de_maquina`.
            Emisor::nuevo(
                DespachoTcp::nuevo(destino, PLAZO_SYSLOG),
                &opciones.nombre,
                interfaz,
            )
        });

    println!("Almacen            : {}", rutas.directorio().display());
    println!(
        "Salida de alertas  : {}",
        opciones
            .syslog
            .as_deref()
            .unwrap_or("NINGUNA; las alertas no salen del equipo y nadie fuera notara si se apaga")
    );
    println!("Estado de arranque : {}", describir(&estado));
    println!("Registro de alertas: {aviso_registro}");

    if estado.exige_alerta() {
        // Dos avisos distintos y no uno. Un formato obsoleto o una instalacion a
        // medias exigen accion administrativa; una supresion o una firma rota
        // exigen respuesta a incidente. Presentarlos igual es como se ensena a
        // un operador a ignorar el segundo.
        if estado.es_manipulacion() {
            println!("  !! MANIPULACION: alguien toco el almacen del agente");
        } else {
            println!("  !  Requiere accion del administrador");
        }
    }
    println!();

    // PA-66. La escucha se abre una vez, fuera del bucle: reabrirla en cada
    // ciclo dejaria una ventana en la que VIS-04 no encuentra a nadie.
    let acceso = opciones
        .grupo_ipc
        .map_or(eje_agente::servicio::Acceso::SoloPropietario, |gid| {
            eje_agente::servicio::Acceso::Grupo(gid)
        });

    let escucha = match Escucha::abrir(&rutas.socket(), acceso) {
        Ok(escucha) => {
            println!("Escucha local      : {}", escucha.ruta().display());
            Some(escucha)
        }
        Err(error) => {
            println!("Escucha local      : NO disponible ({error})");
            // El fallo mas probable desde RPT-067 es que el directorio volatil
            // no exista: bajo `systemd` lo crea `RuntimeDirectory=`, y a mano no
            // lo crea nadie. Se dice cual es, porque el mensaje del sistema
            // habla del socket y no del directorio que falta.
            if !rutas.directorio_socket().is_dir() {
                println!(
                    "  !  El directorio del socket no existe: {}",
                    rutas.directorio_socket().display()
                );
                println!(
                    "     Bajo systemd lo crea RuntimeDirectory=. A mano, usa --directorio-socket."
                );
            }
            None
        }
    };

    // RPT-047, PA-81. `abrir` deja de propagarse con `?`.
    //
    // Antes, un fallo de captura mataba el proceso y se llevaba la escucha que
    // se acababa de abrir tres lineas mas arriba. El momento en que mas falta
    // hace la consola era justo aquel en que el agente ya no estaba.
    //
    // Y sin interfaz no se intenta: con la configuracion rota no hay ninguna que
    // intentar. Se dice distinto porque **es** distinto — una tarjeta que no
    // abre es una averia y esto es una negativa—, y las dos acaban en la misma
    // condicion `capturaNoDisponible`, que es lo unico que el consumidor de
    // alertas necesita saber.
    let mut fuente = match opciones.interfaz.as_deref() {
        Some(interfaz) => match abrir(interfaz) {
            Ok(fuente) => Some(fuente),
            Err(error) => {
                println!("Captura            : NO DISPONIBLE ({error})");
                println!("  !! ESTE SENSOR NO ESTA OBSERVANDO. Se reintenta cada vuelta.");
                None
            }
        },
        None => {
            println!("Captura            : NO SE INTENTA");
            println!("  !! Sin configuracion valida este agente no vigila nada.");
            None
        }
    };

    // PA-68. El ciclo vive en la biblioteca, donde se puede ejercitar N vueltas
    // en pruebas sin levantar un demonio. Aqui queda solo lo que exige una
    // tarjeta de red de verdad: capturar y presentar.
    // RPT-081, PA-135. Una vez, antes del bucle: sus tres campos no cambian
    // durante la ejecucion. Ver `estado_del_agente`.
    let estado_agente = estado_del_agente(opciones.perfil, &estado);

    let mut ciclo = Ciclo::nuevo(rutas.evidencia(), opciones.perfil, registro, emisor);
    ciclo.declarar_intervalo_latido(opciones.intervalo_latido);

    let mut vueltas = 0u64;

    // RPT-072, PA-123. En modo demonio el informe completo por vuelta escribia
    // ~50 lineas por segundo a `journald` en un segmento sin trafico. Observado
    // en maquina real (RPT-069 §2): dos informes enteros entre las 02:17:19 y
    // las 02:17:20.
    let voz = voz_de(opciones.ciclos);

    // Las condiciones de la vuelta anterior, para saber que cambio. No es una
    // lista aparte: se comparan con `enumerar()`, la misma fuente del contrato.
    let mut anteriores: Option<eje_ipc::mensajes::Condiciones> = None;

    // RPT-090, PA-138b. Lo mismo que `anteriores` pero para el inventario: a
    // mitad de vuelta se sirve el de la vuelta anterior, que es el ultimo
    // completo. En la primera no hay, y entonces se rechaza con motivo.
    let mut inventario_anterior: Option<Vec<eje_ipc::mensajes::NodoInventario>> = None;
    let mut ultimo_resumen: Option<i64> = None;

    println!();

    loop {
        // El instante se toma en CADA vuelta. Calculado una sola vez antes del
        // bucle —como estaba— un demonio de dias congelaria el reloj y **ningun
        // marcado caducaria nunca** (RPT-036 §3). Ahora ademas se pasa como
        // argumento, con lo que congelarlo dejo de poder hacerse por descuido.
        let instante = ahora();

        // Reintento por vuelta. Una interfaz que aparece tarde —arranque del
        // sistema, cable reconectado— debe recuperarse sola, sin que nadie
        // reinicie el agente.
        //
        // Sin interfaz no se reintenta: no hay nada que reabrir, y un reintento
        // por vuelta contra una interfaz inexistente seria ruido cada segundo en
        // el modo que existe justo para no hacerlo (RPT-072).
        if fuente.is_none() {
            if let Some(interfaz) = opciones.interfaz.as_deref() {
                if let Ok(recuperada) = abrir(interfaz) {
                    println!("Captura            : RESTABLECIDA");
                    fuente = Some(recuperada);
                }
            }
        }

        let mut observaciones: Vec<Observacion> = Vec::new();
        let mut ilegibles = 0u64;
        let mut perdida: Option<String> = None;
        let mut tramos = Tramos::default();
        let inicio = Instant::now();

        while (observaciones.len() as u64) < opciones.tramas {
            // RPT-084, PA-136. AQUI esta el arreglo. Antes solo se atendia al
            // final de la vuelta, y como la ventana no tiene techo —dura
            // `--tramas` dividido por el ritmo de tramas— la latencia de la
            // consola era la del trafico del segmento: once segundos con un
            // goteo, contra un plazo de cinco (RPT-083 §5 y §6).
            //
            // Se sirve el registro ya persistido y las condiciones de la vuelta
            // ANTERIOR. Cuesta microsegundos y acota la espera al tiempo entre
            // dos tramas en lugar de a la vuelta entera.
            tramos.atender += {
                let marca = Instant::now();
                let atendidas = atender_pendientes(
                    escucha.as_ref(),
                    &ciclo,
                    anteriores.as_ref(),
                    inventario_anterior.as_deref(),
                    &estado_agente,
                );
                if atendidas > 0 && voz == Voz::Detallada {
                    println!("Consultas atendidas a mitad de vuelta: {atendidas}");
                }
                marca.elapsed()
            };

            // La fuente puede no existir (nunca abrio) o desaparecer en marcha
            // (alguien retiro la interfaz). Ninguno de los dos casos propaga:
            // los dos se declaran y se reintentan.
            let intento = match fuente.as_mut() {
                Some(activa) => activa.siguiente(PLAZO),
                None => break,
            };

            let siguiente = match intento {
                Ok(valor) => valor,
                Err(error) => {
                    perdida = Some(error.to_string());
                    break;
                }
            };

            let Some(trama) = siguiente else {
                // Red silenciosa. No es fallo, pero tampoco conviene esperar
                // indefinidamente en un recorrido de comprobacion.
                //
                // En modo demonio se calla: es la linea que mas se repetia, dos
                // veces por segundo, diciendo que un segmento tranquilo esta
                // tranquilo (RPT-072, PA-123).
                if voz == Voz::Detallada {
                    println!("(sin tramas en {PLAZO:?}; se detiene la observacion)");
                }
                break;
            };

            let Some(extraida) = extraer(&trama) else {
                ilegibles = ilegibles.saturating_add(1);
                continue;
            };

            let protocolo = extraida.transporte.and_then(|transporte| {
                let (origen, destino) = transporte.puertos();
                protocolo_de(destino).or_else(|| protocolo_de(origen))
            });

            observaciones.push(Observacion {
                origen: extraida.origen,
                protocolo,
                vlan: extraida.vlan,
            });
        }

        // La perdida del nucleo se traslada al almacen ANTES de concluir nada:
        // sin esto, menos protocolos vistos se leerian como ausencia de riesgo
        // (RPT-018 §4).
        //
        // Sin captura NO se fabrican estadisticas en cero. Cero descartes dice
        // «vista completa», y sin captura no hay vista completa: no hay vista.
        // La ausencia se representa como ausencia y `capturaNoDisponible` es la
        // que carga con el significado (RPT-047 §3).
        // RPT-083, PA-136. La ventana de captura termina AQUI, y esta linea
        // corrige una medicion que llevaba mal desde que se escribio:
        // `inicio.elapsed()` se evaluaba **despues** de `ciclo.vuelta`, asi que
        // el «Tramas observadas: N en X» que se lleva imprimiendo desde RPT-020
        // incluia clasificar, persistir y emitir. Las cifras de la VM —507, 977 y
        // 526 ms— no eran la ventana: eran la ventana mas la vuelta.
        //
        // Un instrumento que mide de mas en el sitio donde ibamos a buscar la
        // causa habria confirmado la hipotesis equivocada.
        tramos.captura = inicio.elapsed();

        let marca = Instant::now();
        let estadisticas = match fuente.as_ref() {
            Some(activa) => Some(activa.estadisticas()?),
            None => None,
        };
        tramos.estadisticas = marca.elapsed();
        if estadisticas
            .as_ref()
            .is_some_and(eje_captura::Estadisticas::hay_perdida)
        {
            ciclo.anotar_perdida();
        }

        if let Some(detalle) = perdida {
            println!("Captura            : PERDIDA EN MARCHA ({detalle})");
            fuente = None;
        }

        // Despues de capturar y en TODAS las vueltas, no solo al cambiar.
        //
        // El ciclo no puede deducir esto del almacen: cero tramas con la
        // captura caida y cero tramas en una red tranquila son el MISMO dato.
        // Un estado que solo se fija al cambiar se queda pegado el dia que
        // alguien olvide el camino de vuelta.
        ciclo.declarar_captura(fuente.is_some());

        // RPT-070, PA-125. En cada vuelta y por el mismo motivo, aunque la
        // escucha se abra una sola vez (PA-66): desde el ciclo, un sensor sin
        // socket y uno al que nadie pregunta son el mismo dato —cero consultas
        // atendidas—, y sin esto las once condiciones dirian que esta sano
        // mientras ninguna consola puede llegar a el.
        ciclo.declarar_escucha(escucha.is_some());

        // RPT-074, PA-79. En cada vuelta aunque el fichero se lea una sola vez, y
        // por lo mismo que las otras dos: un estado que solo se fija al cambiar se
        // queda pegado el dia que alguien olvide el camino de vuelta.
        ciclo.declarar_configuracion(estado_configuracion);

        let mut vistos: BTreeMap<DireccionEnlace, u64> = BTreeMap::new();
        for observacion in &observaciones {
            *vistos.entry(observacion.origen).or_insert(0) += 1;
        }

        let ahora_ms = instante_utc();
        let marca = Instant::now();
        let resultado = ciclo.vuelta(&estado, &observaciones, instante, ahora_ms);
        tramos.vuelta = marca.elapsed();

        let marca = Instant::now();

        if voz == Voz::Detallada {
            println!(
                "Tramas observadas  : {} en {:?}",
                observaciones.len(),
                tramos.captura
            );
            println!("Tramas ilegibles   : {ilegibles}");
            match &estadisticas {
                Some(datos) => println!(
                    "Descartes del nucleo: {} (vista {})",
                    datos.descartadas,
                    if datos.hay_perdida() {
                        "INCOMPLETA"
                    } else {
                        "completa"
                    }
                ),
                // Ni «0» ni «completa»: no se sabe, porque no se miro.
                None => println!("Descartes del nucleo: SIN CAPTURA (no hay vista)"),
            }
            println!("Dispositivos       : {}", vistos.len());
            println!();

            for (mac, cuantas) in vistos.iter().take(20) {
                let indicio = ciclo.almacen().indicio(mac).map_or_else(
                    |error| format!("error: {error}"),
                    |valor| format!("{valor:?}"),
                );

                println!("  {mac:02x?}  tramas={cuantas:<5}  {indicio}");
            }

            if vistos.len() > 20 {
                println!("  ... y {} mas", vistos.len() - 20);
            }

            println!();
            println!(
                "Almacen: {} volatiles, {} pegajosos, saturado={}",
                ciclo.almacen().volatiles(),
                ciclo.almacen().pegajosos(),
                ciclo.almacen().pegajoso_saturado()
            );
            println!();

            presentar(&resultado, ciclo.evidencia());
        } else {
            presentar_cambios(&resultado, anteriores.as_ref(), ciclo.evidencia());

            // La senal de vida local, a la cadencia del latido.
            //
            // Sin ella el silencio absoluto seria correcto y a la vez inservible:
            // un agente atascado y uno vigilando un segmento tranquilo dejarian
            // el mismo diario, que es ninguno. Es el argumento de RPT-052 §1
            // aplicado a `journald` en lugar de a la sala.
            //
            // Y no se ata a `Latido::Emitido` a proposito: un sensor sin colector
            // no late nunca, y ese es justo el caso en que este diario es el
            // unico testigo que existe.
            if ultimo_resumen
                .is_none_or(|ultimo| ahora_ms.saturating_sub(ultimo) >= opciones.intervalo_latido)
            {
                ultimo_resumen = Some(ahora_ms);
                println!(
                    "vivo: vueltas={} dispositivos={} degradado={}",
                    vueltas.saturating_add(1),
                    ciclo.almacen().volatiles(),
                    resultado.condiciones.hay_degradacion()
                );
            }
        }

        tramos.presentar = marca.elapsed();
        anteriores = Some(resultado.condiciones);

        // `inventario_anterior` se asigna DESPUES de atender: `Condiciones` es
        // `Copy` y se puede mover aqui, pero el inventario es un `Vec` y moverlo
        // antes dejaria sin nada que servir al final de esta misma vuelta.

        // Al final del ciclo, sobre lo ya persistido (RPT-034 §4). Una consulta
        // nunca responde con lo que aun vive solo en memoria.
        let marca = Instant::now();
        let atendidas = atender_pendientes(
            escucha.as_ref(),
            &ciclo,
            Some(&resultado.condiciones),
            Some(&resultado.inventario),
            &estado_agente,
        );
        // Se calla en modo demonio: una consola que pregunta cada dos segundos
        // escribiria una linea cada dos segundos, y eso es el mismo defecto con
        // otro disfraz.
        if atendidas > 0 && voz == Voz::Detallada {
            println!("Consultas atendidas al cerrar: {atendidas}");
        }
        // Se ACUMULA: el tramo ya lleva lo gastado dentro del bucle de captura.
        tramos.atender += marca.elapsed();

        inventario_anterior = Some(resultado.inventario);

        // RPT-083, PA-136. Solo con `--ciclos N` finito, que ya significa «hay
        // una persona mirando» (RPT-072). El modo servicio sigue callado.
        if voz == Voz::Detallada {
            tramos.presentar_al_operador();
        }

        vueltas = vueltas.saturating_add(1);
        if opciones.ciclos != 0 && vueltas >= opciones.ciclos {
            break;
        }
        // La linea en blanco separa informes. Sin informe no hay nada que
        // separar, y dos lineas vacias por segundo son el mismo defecto.
        if voz == Voz::Detallada {
            println!();
        }
    }

    Ok(())
}

/// Describe el resultado de cargar el registro, para el operador.
const fn describir_carga(carga: &CargaRegistro) -> &'static str {
    match carga {
        CargaRegistro::Conforme(_) => "verifica",
        CargaRegistro::Truncado { .. } => "TRUNCADO; se perdio la cola por un corte",
        CargaRegistro::ViolacionDetectada { .. } => "NO VERIFICA: alguien lo toco",
    }
}

/// Lee y verifica la configuracion firmada de este sensor. RPT-074, PA-79.
///
/// Se lee **una vez, al arrancar**: una configuracion que cambiara en caliente
/// permitiria a quien escribe el fichero mover el sensor sin que nadie reiniciara
/// nada, y ademas el estado a mitad de vuelta seria distinto del que produjo la
/// vuelta.
///
/// La maquina se compara contra la del NUCLEO y no contra `--nombre`: aquella es
/// la identidad en la sala y un argumento la cambia, asi que compararla contra el
/// fichero permitiria hacer verificar la configuracion de otro sensor con un
/// argumento — que es exactamente lo que `maquina_esperada` impide.
///
/// # Tres estados y no dos
///
/// Que no haya fichero y que lo haya y no valga son cosas distintas, y la
/// diferencia se pierde si se resuelven con un `bool`. Es RPT-006 §4.
///
/// El motivo viaja dentro de [`Configuracion::NoVerifica`] y no en `Condiciones`,
/// que describe lo que es verdad ahora con una forma uniforme. Sin el, el tecnico
/// que llega a la planta ve «no verifica» y no sabe si es una firma rota, una
/// clave rotada o una configuracion de otro sensor.
fn leer_configuracion(
    rutas: &RutasAlmacen,
    maquina: &str,
    aceptada: guardian_cc::inventario::Centinela,
) -> Configuracion {
    let ruta = std::path::Path::new(guardian_cc::configuracion::RUTA_CONFIGURACION);

    let Ok(bytes) = std::fs::read(ruta) else {
        // No hay fichero. No se distingue «no existe» de «no se puede leer» a
        // proposito: las dos significan que este agente no tiene configuracion
        // firmada que obedecer, y el remedio es el mismo.
        return Configuracion::Ausente;
    };

    // Hay configuracion y hace falta la clave del cliente para juzgarla. Sin
    // clave NO se concluye «ausente»: hay un fichero, y decir que no lo hay
    // mandaria a emitir uno cuando lo que falta es aprovisionar la clave.
    let clave = match std::fs::read(rutas.clave_operativa())
        .ok()
        .and_then(|bytes| analizar_clave(&bytes, DominioClave::Cliente).ok())
    {
        Some(clave) => clave,
        None => {
            return Configuracion::NoVerifica(
                "hay configuracion firmada y no hay clave con que verificarla".to_owned(),
            );
        }
    };

    match guardian_cc::configuracion::analizar(&bytes, &clave, maquina, aceptada) {
        Ok(valores) => Configuracion::Firmada(Box::new(valores)),
        Err(motivo) => Configuracion::NoVerifica(motivo.to_string()),
    }
}

/// Atiende lo que haya pendiente en el socket, y devuelve cuantas se sirvieron.
///
/// RPT-084, PA-136. Se llama **dos veces por vuelta**, y esa es la correccion:
///
/// - **entre trama y trama**, con las condiciones de la vuelta anterior;
/// - **al final**, con las recien calculadas.
///
/// # Por que se puede servir a mitad de vuelta
///
/// Lo que se entrega es el registro **ya persistido** y las condiciones de la
/// ultima vuelta completa. RPT-034 §4 exige exactamente eso —nunca lo que vive
/// solo en memoria— y aqui se cumple sin tocarlo.
///
/// Parecia un intercambio entre latencia y frescura, y no lo es: atender solo al
/// final entrega un dato fresco **que llega hasta once segundos tarde**
/// (RPT-083 §5), asi que la edad del dato en manos del operador es la misma. La
/// diferencia es que una version responde y la otra se cuelga.
///
/// # El coste
///
/// Medido: entre 12 y 312 microsegundos. Doscientas llamadas son cuatro
/// milisegundos sobre una vuelta de once segundos. No hay nada que optimizar aqui,
/// y por eso no hace falta ni un hilo aparte ni acortar la ventana.
///
/// # Generico sobre el despacho, y sin usarlo
///
/// `Ciclo<D>` lo es porque el emisor de syslog es intercambiable en pruebas. Aqui
/// solo se leen el registro y la ruta de evidencia, asi que `D` viaja sin que a
/// esta funcion le importe cual sea. La cota `Despacho` la exige el `impl` del
/// ciclo, no esto.
fn atender_pendientes<D: eje_agente::salida::Despacho>(
    escucha: Option<&Escucha>,
    ciclo: &Ciclo<D>,
    condiciones: Option<&eje_ipc::mensajes::Condiciones>,
    inventario: Option<&[eje_ipc::mensajes::NodoInventario]>,
    estado_agente: &EstadoAgente,
) -> usize {
    let Some(escucha) = escucha else {
        return 0;
    };

    escucha.atender(&mut Manejadores {
        registro: ciclo.registro(),
        condiciones,
        inventario,
        evidencia: ciclo.evidencia(),
        estado_agente,
    })
}

/// Cuanto tarda cada tramo de una vuelta. RPT-083, PA-136.
///
/// # Por que se mide, y por que aqui
///
/// En la VM, una consulta por el socket tardo entre 444 y 983 ms sobre un socket
/// de dominio Unix local, donde deberia ser de un digito. La causa **parece** ser
/// que el agente atiende al final del ciclo (RPT-034 §4) y cada ciclo espera
/// hasta [`PLAZO`] por trama — pero eso es aritmetica, no observacion, y esta
/// semana ha dejado claro lo que cuesta confundirlas.
///
/// Se imprime **solo con `--ciclos N` finito**, que ya significa «hay una persona
/// mirando» (RPT-072). El modo servicio sigue callado: una bandera nueva seria
/// una segunda forma de decir lo que `--ciclos` ya dice.
///
/// # La hipotesis que da miedo
///
/// `ciclo.vuelta` incluye la emision a syslog, y `DespachoTcp` lleva
/// [`PLAZO_SYSLOG`] = 3 s. Con `colector = ""` no hay emisor y ese tramo se salta
/// entero, que es como se midio en la VM. **Un sensor con un colector
/// inalcanzable gastaria hasta tres segundos por vuelta**, y la consola quedaria
/// cerca del plazo de 5 s de `ESPERA_MAXIMA_MS`.
///
/// El momento en que el SIEM se cae es exactamente el momento en que alguien abre
/// la consola a mirar. Si esa hipotesis se confirma, lo medido en la VM era el
/// caso bueno.
#[derive(Debug, Default, Clone, Copy)]
struct Tramos {
    /// La ventana de observacion. Acotada por `--tramas` o por [`PLAZO`].
    captura: std::time::Duration,
    /// Preguntar al nucleo cuantas tramas descarto.
    estadisticas: std::time::Duration,
    /// Clasificar, persistir, sellar y **emitir**. El tramo sospechoso.
    vuelta: std::time::Duration,
    /// Escribir el informe por pantalla.
    presentar: std::time::Duration,
    /// Atender las consultas que hubiera pendientes en el socket.
    atender: std::time::Duration,
}

impl Tramos {
    /// Imprime el desglose, con el total y el tramo dominante.
    ///
    /// El dominante se calcula y se nombra en lugar de dejar cinco cifras para
    /// que alguien las compare a ojo. Leer mal un desglose es tan facil como leer
    /// mal un diario, y esta semana ya paso.
    fn presentar_al_operador(self) {
        let etiquetas = [
            ("captura", self.captura),
            ("estadisticas", self.estadisticas),
            ("vuelta", self.vuelta),
            ("presentar", self.presentar),
            ("atender", self.atender),
        ];

        let total: std::time::Duration = etiquetas.iter().map(|(_, cuanto)| *cuanto).sum();

        println!();
        println!("  Tramos de la vuelta          : {total:?} en total");
        for (nombre, cuanto) in etiquetas {
            let parte = if total.is_zero() {
                0
            } else {
                // Entero: una cifra con decimales invita a comparar ruido.
                cuanto.as_micros().saturating_mul(100) / total.as_micros().max(1)
            };
            println!("    {nombre:<14}: {cuanto:>12?}  {parte:>3}%");
        }

        if let Some((nombre, cuanto)) = etiquetas.iter().max_by_key(|(_, cuanto)| *cuanto) {
            println!("    domina '{nombre}' con {cuanto:?}");
        }
    }
}

/// Traduce el perfil del **dominio** al del **cable**.
///
/// RPT-081, PA-135.
///
/// # Por que hay dos tipos, y por que esto no es un `impl From`
///
/// `guardian_cc::PerfilSegmento` y `eje_ipc::mensajes::PerfilSegmento` tienen el
/// mismo nombre y las mismas variantes, y para el compilador son tipos ajenos.
/// **No es un descuido:** `eje-ipc` depende de `thiserror` y `serde` y de nada
/// mas, porque la capa de transporte no debe depender del nucleo de dominio. Los
/// dos se comprueban contra `contrato-ipc.toml` por separado, que es lo que
/// impide que diverjan.
///
/// Un `impl From` tendria que vivir en uno de esos dos crates —regla del
/// huerfano— y eso obligaria a invertir esa dependencia para ahorrar cuatro
/// lineas. La traduccion es una decision del agente, que es quien conoce los dos
/// lados, asi que vive aqui.
///
/// El `match` es exhaustivo: anadir un perfil **no compila** hasta que alguien
/// decida como viaja. Lo que el `match` no impide es traducirlo **mal** —
/// `Corporativo => Ot` compila igual de bien—, y de eso se ocupa
/// `el_perfil_no_se_cruza_al_pasar_al_cable`.
const fn perfil_en_el_cable(perfil: PerfilSegmento) -> eje_ipc::mensajes::PerfilSegmento {
    match perfil {
        PerfilSegmento::Corporativo => eje_ipc::mensajes::PerfilSegmento::Corporativo,
        PerfilSegmento::Ot => eje_ipc::mensajes::PerfilSegmento::Ot,
    }
}

/// Compone el estado resumido que responde `obtener-estado-agente`.
///
/// RPT-081, PA-135.
///
/// # `respuestaAutomatica` es una conjuncion, y no un campo
///
/// El encargo decia atarla a [`EstadoArranque::admite_contencion_automatica`].
/// **Es la mitad**, y la mitad que falta es la peligrosa.
///
/// Quien decide de verdad si el agente contiene solo son DOS guardas
/// independientes:
///
/// - `PerfilSegmento::permite_respuesta_automatica` — el perfil `ot` **nunca** la
///   admite. IEC 62443 ordena las prioridades de una planta al reves que TI:
///   una contencion automatica que detiene una linea **es** el incidente, no la
///   respuesta al incidente. Esta guarda ya decide en `evaluar`.
/// - `EstadoArranque::admite_contencion_automatica` — con el inventario
///   suprimido o sin verificar no se contiene nada, diga lo que diga el perfil.
///
/// Informar solo la segunda haria que un sensor de planta —perfil `ot`, almacen
/// impecable— dijera `respuestaAutomatica: true`. Le estaria diciendo al
/// operador que ese sensor actua solo cuando no lo hara jamas, que es la
/// direccion equivocada en la que mentir.
///
/// # La guarda que NO se consulta, y por que se dice aqui
///
/// `boveda::VigenciaReglas::permite_respuesta_automatica` existe, esta probada, y
/// **no la llama nadie**. La descripcion de este canal en el contrato dice «segun
/// vigencia de reglas», asi que el nombre promete una tercera guarda que hoy no
/// se evalua. No se incluye inventando un valor: no hay distribucion de reglas,
/// asi que «vigentes» seria una suposicion disfrazada de dato. Queda en PA-137.
fn estado_del_agente(perfil: PerfilSegmento, estado: &EstadoArranque) -> EstadoAgente {
    EstadoAgente {
        version: VERSION.to_owned(),
        perfil: perfil_en_el_cable(perfil),
        respuesta_automatica: perfil.permite_respuesta_automatica()
            && estado.admite_contencion_automatica(),
    }
}

/// Avanza la marca de agua **antes** de obedecer, y no despues.
///
/// RPT-078, PA-79 paso 5. Es la mitad del mecanismo que no se ve: sin ella, el
/// agente compara contra una marca que nunca sube y la comprobacion de frescura
/// no rechaza nada jamas. Un centinela que no avanza es un centinela decorativo.
///
/// # Un fallo al anotar convierte la configuracion en no verificable
///
/// No se obedece «de todos modos». Si la marca no se puede escribir, el proximo
/// arranque no sabra que se llego a ver esta secuencia, y obedecer ahora dejaria
/// exactamente la ventana de reversion que esto cierra. Se degrada a
/// [`Configuracion::NoVerifica`] con el motivo del disco, que es un estado que el
/// sensor ya sabe declarar y que la sala ya sabe leer.
///
/// Anotar una secuencia que ya estaba anotada no cuesta nada y se hace igual: la
/// alternativa —comparar antes de escribir— seria una segunda copia de la regla
/// de frescura, viviendo lejos de la primera.
fn anotar_configuracion(rutas: &RutasAlmacen, configuracion: Configuracion) -> Configuracion {
    let Configuracion::Firmada(valores) = &configuracion else {
        return configuracion;
    };

    match aceptar_configuracion(rutas, valores.secuencia) {
        Ok(_) => configuracion,
        Err(error) => Configuracion::NoVerifica(format!(
            "la configuracion verifica y no se pudo anotar su secuencia ({error}); \
             obedecerla sin dejar constancia reabriria la ventana de reversion"
        )),
    }
}

/// Cuanto habla el agente por su salida estandar.
///
/// RPT-072, PA-123. El informe completo por vuelta es **presentacion para una
/// persona delante de un terminal**, y el modo demonio lo ejecutaba dos veces por
/// segundo contra `journald`. Es la misma familia que el reloj congelado y la
/// reemision del historial: codigo correcto escrito para ejecutarse una vez,
/// ejecutandose muchas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Voz {
    /// Informe completo cada vuelta. `--ciclos N` finito: el recorrido de
    /// comprobacion de siempre, que alguien lee mientras ocurre.
    Detallada,
    /// Solo lo que cambia, mas una senal de vida a la cadencia del latido.
    /// `--ciclos 0`, que es el servicio.
    SoloCambios,
}

/// Si esta ejecucion es **el servicio** y no un recorrido de comprobacion.
///
/// `--ciclos 0` es el servicio. La distincion no se anade como bandera nueva a
/// proposito: ya existe una opcion que dice exactamente eso, y dos formas de
/// decir lo mismo se contradicen el dia que alguien cambie una.
///
/// # De aqui cuelgan dos decisiones, y conviene que cuelguen de la misma
///
/// Cuanto habla ([`voz_de`], RPT-072) y **si morir ante una configuracion
/// ausente** ([`resolver`], RPT-080). Las dos preguntan lo mismo —«¿hay una
/// persona delante?»— y por eso comparten esta funcion en lugar de comparar
/// `ciclos == 0` cada una por su cuenta.
const fn es_servicio(ciclos: u64) -> bool {
    ciclos == 0
}

/// Cuanto habla el agente segun cuantas vueltas vaya a dar.
const fn voz_de(ciclos: u64) -> Voz {
    if es_servicio(ciclos) {
        Voz::SoloCambios
    } else {
        Voz::Detallada
    }
}

/// Que condiciones cambiaron entre dos vueltas, con su valor nuevo.
///
/// RPT-072, PA-123. La **decision**, separada de la presentacion: asi se puede
/// probar sin capturar `stdout`, que es la misma razon por la que el ciclo vive
/// en la biblioteca y no aqui.
///
/// `anteriores` en `None` es la primera vuelta: se devuelven las condiciones
/// **activas**, que son el estado de partida y no una transicion inventada. Las
/// apagadas no se anuncian nunca al arrancar, o el arranque de un sensor sano
/// produciria once lineas diciendo que no pasa nada.
fn condiciones_que_cambiaron(
    anteriores: Option<&eje_ipc::mensajes::Condiciones>,
    ahora: &eje_ipc::mensajes::Condiciones,
) -> Vec<(&'static str, bool)> {
    let previas = anteriores.map(eje_ipc::mensajes::Condiciones::enumerar);

    ahora
        .enumerar()
        .into_iter()
        .enumerate()
        .filter_map(|(indice, (nombre, activa))| {
            let antes = previas.is_some_and(|lista| lista[indice].1);
            (antes != activa).then_some((nombre, activa))
        })
        .collect()
}

/// Imprime unicamente lo que cambio respecto de la vuelta anterior.
///
/// RPT-072, PA-123.
///
/// # Que se dice y que se calla
///
/// Se dice todo suceso —alertas anexadas, perdidas, fallos de persistencia,
/// rotacion— y **toda transicion de condicion**. Se calla el recuento de tramas,
/// la tabla de dispositivos y el estado de las once condiciones cuando ninguna se
/// movio, que es lo que ocupaba el 95% del volumen.
///
/// # Por que las transiciones se derivan y no se listan
///
/// Se comparan las dos `enumerar()` posicion a posicion. Una lista de condiciones
/// escrita aqui seria el sexto indice a mano de la semana, y ya sabemos como
/// acaban: la de `presentar` se quedo en siete de diez —sin `capturaNoDisponible`,
/// la mas grave— hasta que alguien lo leyo en una consola de verdad (PA-114).
///
/// `anteriores` en `None` es la primera vuelta: se anuncian las condiciones
/// activas, que es el estado de partida y no una transicion inventada.
fn presentar_cambios(
    resultado: &Resultado,
    anteriores: Option<&eje_ipc::mensajes::Condiciones>,
    evidencia: &std::path::Path,
) {
    for (nombre, activa) in condiciones_que_cambiaron(anteriores, &resultado.condiciones) {
        if activa {
            println!("!! condicion ENCENDIDA: {nombre}");
        } else {
            println!("   condicion apagada   : {nombre}");
        }
    }

    if !resultado.anexadas.is_empty() {
        println!(
            "alertas anexadas: {} (registro en {})",
            resultado.anexadas.len(),
            evidencia.display()
        );
    }

    if resultado.perdidas > 0 {
        println!(
            "!! {} amenazas detectadas en esta vuelta NO se pudieron anotar.",
            resultado.perdidas
        );
    }

    if let Some(motivo) = &resultado.fallo_persistencia {
        println!("!! No se pudo persistir el registro: {motivo}");
        println!("   Las alertas de esta vuelta NO sobreviven al reinicio.");
    }

    if let Some(archivado) = &resultado.rotado {
        println!("segmento archivado: {}", archivado.display());
    }

    // De los cuatro estados del latido solo este es una noticia. `Emitido` y
    // `NoTocaba` son funcionamiento normal, y `SinColector` ya lo dice su
    // condicion, que sale por la lista de arriba en cuanto se enciende.
    if resultado.latido == Latido::NoSePudo {
        println!("!! Tocaba latir y el latido NO salio.");
        println!("   Para la sala, este sensor es indistinguible de uno muerto.");
    }
}

/// Imprime lo que la vuelta produjo.
///
/// Presentacion y nada mas. La decision ya la tomo [`Ciclo::vuelta`]; separarlo
/// es lo que permite que el ciclo se pruebe sin capturar `stdout`.
fn presentar(resultado: &Resultado, evidencia: &std::path::Path) {
    println!("  Con marcado firmado          : {}", resultado.con_marcado);
    println!("  Contenibles sin intervencion : {}", resultado.contenibles);
    println!("  Requieren humano o prohibidos: {}", resultado.escalados);
    println!();

    println!(
        "  Alertas anexadas a ALM-01    : {}",
        resultado.anexadas.len()
    );
    for suceso in resultado.anexadas.iter().take(5) {
        println!(
            "    asiento {:<4} {}  {:?}",
            suceso.asiento, suceso.dispositivo, suceso.clase
        );
    }
    if resultado.anexadas.len() > 5 {
        println!("    ... y {} mas", resultado.anexadas.len() - 5);
    }

    if resultado.persistido {
        println!("  Registro persistido          : {}", evidencia.display());
    }
    if resultado.sellado {
        // PA-64. El extremo salio de la maquina. Es lo unico que un equipo
        // comprometido no puede deshacer despues.
        println!("  Extremo atestiguado fuera    : si");
    }
    // PA-104. Los cuatro casos se dicen por separado porque en el cable los tres
    // primeros suenan igual —silencio— y solo uno es una averia.
    match resultado.latido {
        Latido::Emitido => println!("  Latido enviado al colector   : si"),
        Latido::NoTocaba => {}
        Latido::SinColector => {
            println!("  Latido enviado al colector   : NO HAY COLECTOR");
            println!("     Este sensor no late. Nadie fuera puede notar que se apaga.");
        }
        Latido::NoSePudo => {
            println!("  !! Tocaba latir y el latido NO salio.");
            println!("     Para la sala, este sensor es indistinguible de uno muerto.");
        }
    }
    if let Some(archivado) = &resultado.rotado {
        println!("  Segmento archivado           : {}", archivado.display());
    }
    if let Some(motivo) = &resultado.fallo_persistencia {
        println!("  !! No se pudo persistir el registro: {motivo}");
        println!("     Las alertas de esta vuelta NO sobreviven al reinicio.");
    }

    let estados = &resultado.condiciones;
    println!();
    println!("  Condiciones vigentes:");

    // RPT-058, PA-114. Se recorre `enumerar` en vez de escribir la lista aqui.
    // Escrita a mano se quedo en SIETE de diez —sin `capturaNoDisponible`, que es
    // la mas grave— y nadie lo vio hasta leerlo en una consola de verdad. Un
    // resumen que omite una condicion activa dice que todo va bien exactamente
    // igual que uno que la muestra apagada.
    for (nombre, activa) in estados.enumerar() {
        println!("    {nombre:<22}: {activa}");
    }

    if estados.registro_saturado {
        println!();
        println!("  !! El registro esta LLENO. Este sensor ya no anota amenazas.");
        println!("     No es manipulacion: nadie toco nada. Hace falta rotar el");
        println!("     registro para que vuelva a registrar (PA-59).");
    }
    if resultado.perdidas > 0 {
        println!(
            "  !! {} amenazas detectadas en esta vuelta NO se pudieron anotar.",
            resultado.perdidas
        );
    }

    if estados.salida_no_disponible {
        println!();
        println!("  !! El SIEM del cliente NO se esta enterando de nada.");
        println!("     Las alertas siguen en ALM-01, que es para lo que existe.");
        println!("     Y el extremo del registro no esta saliendo del equipo: mientras");
        println!("     dure, un recorte del registro local no dejaria testigo (PA-64).");
    }
}

/// Instante actual en milisegundos desde la epoca, con signo.
///
/// ALM-01 lo quiere con signo y en milisegundos. Un reloj anterior a la epoca
/// produce un valor negativo, que es representable y honesto; devolver 0 seria
/// inventar una fecha.
fn instante_utc() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |transcurrido| {
            i64::try_from(transcurrido.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod pruebas_voz {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use eje_ipc::mensajes::Condiciones;

    use super::{Voz, condiciones_que_cambiaron, voz_de};

    fn calma() -> Condiciones {
        Condiciones {
            inventario_suprimido: false,
            inventario_no_verifica: false,
            observacion_saturada: false,
            captura_con_perdida: false,
            captura_no_disponible: false,
            accion_administrativa: false,
            salida_no_disponible: false,
            sin_colector: false,
            escucha_no_disponible: false,
            configuracion_sin_firmar: false,
            configuracion_no_verifica: false,
            registro_saturado: false,
            evidencia_en_riesgo: false,
        }
    }

    /// El servicio calla; el recorrido de comprobacion habla.
    #[test]
    fn el_modo_continuo_es_el_que_se_calla() {
        assert_eq!(voz_de(0), Voz::SoloCambios, "--ciclos 0 es el servicio");
        assert_eq!(voz_de(1), Voz::Detallada, "el valor por omision es de mano");
        assert_eq!(voz_de(500), Voz::Detallada);
    }

    /// En calma sostenida no se dice absolutamente nada.
    ///
    /// RPT-072, PA-123. Es la afirmacion entera del punto: un segmento tranquilo
    /// producia ~50 lineas por segundo, y lo que tiene que producir es ninguna.
    #[test]
    fn una_vuelta_sin_cambios_no_dice_nada() {
        let estable = calma();
        assert!(condiciones_que_cambiaron(Some(&estable), &estable).is_empty());
    }

    /// La primera vuelta anuncia lo que esta encendido, y solo eso.
    ///
    /// Sin el filtro, un sensor sano escupiria once lineas al arrancar diciendo
    /// que no pasa nada, y quien las lea aprendera a saltarselas.
    #[test]
    fn la_primera_vuelta_anuncia_lo_activo_y_calla_lo_apagado() {
        let arranque = Condiciones {
            accion_administrativa: true,
            ..calma()
        };

        assert_eq!(
            condiciones_que_cambiaron(None, &arranque),
            vec![("accionAdministrativa", true)]
        );
    }

    /// Encenderse y apagarse son dos noticias, no una.
    #[test]
    fn el_encendido_y_el_apagado_se_anuncian_los_dos() {
        let antes = calma();
        let degradado = Condiciones {
            escucha_no_disponible: true,
            ..calma()
        };

        assert_eq!(
            condiciones_que_cambiaron(Some(&antes), &degradado),
            vec![("escuchaNoDisponible", true)],
            "encenderse es una noticia"
        );
        assert_eq!(
            condiciones_que_cambiaron(Some(&degradado), &antes),
            vec![("escuchaNoDisponible", false)],
            "recuperarse tambien: sin esto el diario se queda con la mala noticia"
        );
    }

    /// Varias a la vez salen todas, en el orden del contrato.
    #[test]
    fn varias_transiciones_a_la_vez_salen_todas() {
        let antes = calma();
        let mal = Condiciones {
            captura_no_disponible: true,
            escucha_no_disponible: true,
            ..calma()
        };

        assert_eq!(
            condiciones_que_cambiaron(Some(&antes), &mal),
            vec![("capturaNoDisponible", true), ("escuchaNoDisponible", true)]
        );
    }
}

#[cfg(test)]
mod pruebas_obediencia {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use guardian_cc::PerfilSegmento;
    use guardian_cc::configuracion::Valores;

    use guardian_cc::arranque::{RutasAlmacen, cargar_centinela};

    use super::{
        Argumentos, Configuracion, ErrorAgente, OPCIONES, Opcion, anotar_configuracion,
        leer_opciones_de, resolver, rutas_de_instalacion,
    };

    fn argv(pares: &[&str]) -> Vec<String> {
        pares.iter().map(|texto| (*texto).to_owned()).collect()
    }

    fn leidos(pares: &[&str]) -> Argumentos {
        leer_opciones_de(&argv(pares)).expect("los argumentos de la prueba son validos")
    }

    /// Una configuracion firmada plausible, distinta en cada campo de lo que la
    /// linea de ordenes pondria. Si coincidieran, una prueba podria pasar por el
    /// motivo equivocado.
    fn valores() -> Valores {
        Valores {
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

    fn firmada() -> Configuracion {
        Configuracion::Firmada(Box::new(valores()))
    }

    /// Con configuracion firmada, **manda ella y nada mas que ella**.
    ///
    /// Es la afirmacion del paso 4b entera. Hasta ayer el agente leia esto,
    /// imprimia «firmada y verificada», y a continuacion corria con lo que
    /// dijera el `ExecStart`.
    #[test]
    fn con_configuracion_firmada_los_parametros_salen_de_la_firma() {
        let efectivas = resolver(leidos(&["--ciclos", "0"]), &firmada()).expect("debe resolver");

        assert_eq!(efectivas.interfaz.as_deref(), Some("eth0"));
        assert_eq!(efectivas.perfil, PerfilSegmento::Ot);
        assert_eq!(efectivas.nombre, "sensor-planta-3");
        assert_eq!(efectivas.intervalo_latido, 60_000);
        assert_eq!(efectivas.grupo_ipc, Some(1000));
        assert_eq!(efectivas.syslog.as_deref(), Some("siem.hospital:514"));
        // `--ciclos` no lo dicta: sigue siendo de quien arranca el proceso.
        assert_eq!(efectivas.ciclos, 0);
    }

    /// **Toda** bandera dictada aborta el arranque, recorriendo la tabla.
    ///
    /// RPT-074 §10. Escrita como bucle sobre [`OPCIONES`] y no como una lista de
    /// casos: una opcion nueva marcada `dictada` queda cubierta el mismo dia que
    /// se anade. Una lista de casos escrita a mano seria el septimo indice de la
    /// semana, y ya sabemos como acaban — se quedan cortos sin fallar.
    #[test]
    fn ninguna_bandera_dictada_se_admite_con_configuracion_firmada() {
        for opcion in OPCIONES.iter().filter(|opcion| opcion.dictada) {
            let Opcion {
                bandera, ejemplo, ..
            } = *opcion;

            let resultado = resolver(leidos(&[bandera, ejemplo]), &firmada());

            match resultado {
                Err(ErrorAgente::ArgumentoDictado(culpable, _)) => {
                    assert_eq!(culpable, bandera, "el error acusa a otra bandera");
                }
                Err(otro) => panic!("'{bandera}' fallo por otro motivo: {otro}"),
                Ok(_) => panic!(
                    "'{bandera}' se declara dictada y aun asi el agente arrancaria \
                     con ella: la firma no vale nada si un argumento la deja sin efecto"
                ),
            }
        }
    }

    /// Y se rechaza **aunque el argumento diga exactamente lo mismo**.
    ///
    /// Comparar obligaria a decidir que significa «igual» para cada tipo, y cada
    /// una de esas decisiones es un sitio donde colar un valor que pasa por igual
    /// sin serlo. La regla sin grados no tiene ese sitio.
    #[test]
    fn un_argumento_que_coincide_con_lo_firmado_tambien_aborta() {
        assert!(matches!(
            resolver(leidos(&["--interfaz", "eth0"]), &firmada()),
            Err(ErrorAgente::ArgumentoDictado("--interfaz", _))
        ));
    }

    /// Lo que la firma no dicta sigue admitiendose, tambien recorriendo la tabla.
    ///
    /// La otra mitad, y hace falta: sin ella, marcar todo como dictado pasaria la
    /// prueba de arriba y dejaria una unidad de `systemd` que no puede arrancar.
    #[test]
    fn lo_que_la_firma_no_dicta_se_sigue_admitiendo() {
        for opcion in OPCIONES.iter().filter(|opcion| !opcion.dictada) {
            let Opcion {
                bandera, ejemplo, ..
            } = *opcion;

            assert!(
                resolver(leidos(&[bandera, ejemplo]), &firmada()).is_ok(),
                "'{bandera}' no la dicta la firma y aun asi impide arrancar"
            );
        }
    }

    /// Sin configuracion, la flota de hoy sigue arrancando igual.
    ///
    /// El paso 4b no rompe ningun despliegue existente, y esta es la prueba de
    /// esa frase: no hay ninguna maquina con configuracion firmada todavia.
    #[test]
    fn sin_configuracion_manda_la_linea_de_ordenes() {
        let efectivas = resolver(
            leidos(&["--interfaz", "eth9", "--perfil", "ot"]),
            &Configuracion::Ausente,
        )
        .expect("sin configuracion se arranca por argumentos");

        assert_eq!(efectivas.interfaz.as_deref(), Some("eth9"));
        assert_eq!(efectivas.perfil, PerfilSegmento::Ot);
    }

    /// Sin configuracion y sin interfaz, **el servicio arranca y lo declara**.
    ///
    /// RPT-080, PA-133. Es el sensor recien instalado y sin aprovisionar, que es
    /// como llega **toda** maquina nueva. Antes moria con un error de uso, y bajo
    /// `Restart=always` eso no es una averia visible: es un bucle de reinicios.
    /// Se observo ocurriendo 350 veces seguidas sin que nada fuera del diario
    /// local se enterase (RPT-079 §11).
    #[test]
    fn sin_configuracion_el_servicio_arranca_y_declara_en_lugar_de_morir() {
        let efectivas = resolver(leidos(&["--ciclos", "0"]), &Configuracion::Ausente)
            .expect("un sensor sin aprovisionar tiene que arrancar y declararlo");

        assert_eq!(
            efectivas.interfaz, None,
            "no hay nada que vigilar, y disfrazarlo de interfaz seria mentir"
        );
    }

    /// Pero a mano, sin interfaz, se explica el uso.
    ///
    /// La distincion la hace `--ciclos`, que ya separa el servicio del recorrido
    /// de comprobacion desde RPT-072. Ahi hay una persona delante que acaba de
    /// teclear algo incompleto: darle un sensor que no observa, en lugar de
    /// decirle que falta, seria obedecer la letra y perder el sentido.
    #[test]
    fn a_mano_y_sin_interfaz_se_explica_el_uso() {
        for argv in [vec![], vec!["--ciclos", "1"], vec!["--tramas", "10"]] {
            assert!(
                matches!(
                    resolver(leidos(&argv), &Configuracion::Ausente),
                    Err(ErrorAgente::Uso)
                ),
                "con {argv:?} hay alguien delante esperando la linea de uso"
            );
        }
    }

    /// **Ningun estado de configuracion puede impedir que el servicio arranque.**
    ///
    /// RPT-080, PA-133. Esta es la barrera, y las dos de arriba son sus casos.
    /// El defecto no fue equivocarse en una rama: fue que RPT-077 §5 razono
    /// «arrancar y declarar en lugar de morir» para la firma **rota** y no lo
    /// aplico a la firma **ausente**. Una regla que se aplica caso por caso se
    /// olvida en el siguiente caso.
    ///
    /// El `match` de `cada_estado` es exhaustivo a proposito: anadir una variante
    /// a [`Configuracion`] **no compila** hasta que alguien decida si el servicio
    /// sigue arrancando con ella.
    #[test]
    fn el_servicio_arranca_diga_lo_que_diga_la_configuracion() {
        fn cada_estado() -> Vec<Configuracion> {
            match &Configuracion::Ausente {
                Configuracion::Firmada(_)
                | Configuracion::Ausente
                | Configuracion::NoVerifica(_) => {}
            }

            vec![
                firmada(),
                Configuracion::Ausente,
                Configuracion::NoVerifica("la firma no verifica".to_owned()),
            ]
        }

        for configuracion in cada_estado() {
            assert!(
                resolver(leidos(&["--ciclos", "0"]), &configuracion).is_ok(),
                "el servicio no arranca con esta configuracion, y morir bajo \
                 Restart=always es un bucle de reinicios que nadie ve"
            );
        }
    }

    /// Con la firma rota **no manda nadie**, y menos que nadie los argumentos.
    ///
    /// Es la prueba que sujeta la decision dura del paso. Caer a la linea de
    /// ordenes cuando la configuracion no verifica le bastaria a quien pudo tocar
    /// el fichero para recuperar el mando: romperlo y volver al `ExecStart` que
    /// controla. Aqui se pasan `--interfaz` y `--syslog` **a proposito**, para
    /// comprobar que no se cuelan por la puerta de atras.
    #[test]
    fn con_la_firma_rota_los_argumentos_tampoco_mandan() {
        let efectivas = resolver(
            leidos(&["--interfaz", "eth9", "--syslog", "10.0.0.9:514"]),
            &Configuracion::NoVerifica("la firma no verifica".to_owned()),
        )
        .expect("declarar la averia no puede depender de que los argumentos falten");

        assert_eq!(
            efectivas.interfaz, None,
            "un atacante recuperaria la interfaz rompiendo el fichero"
        );
        assert_eq!(
            efectivas.syslog, None,
            "y el destino de las alertas, que es peor"
        );
        assert_eq!(efectivas.grupo_ipc, None);
    }

    /// Pero **arranca**, en lugar de morirse.
    ///
    /// Un agente que se cae con `Restart=always` es un bucle de reinicios, y para
    /// la sala un sensor muerto es indistinguible de un cable cortado. Vivo y
    /// declarando es un diagnostico; ademas es lo que hace alcanzable la
    /// condicion `configuracionNoVerifica`, que si no seria un mecanismo sin
    /// cablear recien estrenado.
    #[test]
    fn con_la_firma_rota_el_agente_arranca_y_lo_declara() {
        assert!(
            resolver(
                leidos(&[]),
                &Configuracion::NoVerifica("firma invalida".to_owned())
            )
            .is_ok()
        );
    }

    /// Cada campo de la configuracion firmada tiene su bandera dictada.
    ///
    /// La desestructuracion es la barrera: anadir un campo a `Valores` **no
    /// compila** hasta que alguien decida si es un parametro del sensor —y le
    /// ponga su bandera— o una defensa como la secuencia. Sin esto, un campo
    /// nuevo seria configurable por la linea de ordenes y nadie se enteraria.
    #[test]
    fn cada_campo_firmado_tiene_bandera_y_ninguna_sobra() {
        let Valores {
            // No son parametros del sensor: uno impide la reversion y el otro
            // dice a que maquina va dirigida la configuracion.
            secuencia: _,
            maquina_esperada: _,
            // Y estos seis si lo son.
            interfaz: _,
            perfil: _,
            colector: _,
            intervalo_latido_ms: _,
            grupo_ipc: _,
            nombre: _,
        } = valores();

        const PARAMETROS_FIRMADOS: usize = 6;

        assert_eq!(
            OPCIONES.iter().filter(|opcion| opcion.dictada).count(),
            PARAMETROS_FIRMADOS,
            "la configuracion firmada y la tabla de opciones dejaron de contar lo mismo"
        );
    }

    /// La marca de agua **avanza de verdad**, y queda en disco.
    ///
    /// RPT-078, PA-79 paso 5. Es la mitad que no se ve y la que decide si todo
    /// esto sirve para algo: la comprobacion de frescura de `analizar` compara
    /// contra una marca, y si nadie la sube, no rechaza nada nunca. Un centinela
    /// que no avanza es un centinela decorativo — que es la familia de defectos
    /// de esta casa, cometida en el mecanismo que viene a cerrarla.
    #[test]
    fn anotar_una_configuracion_sube_la_marca_en_disco() {
        let directorio = std::env::temp_dir().join("eje-latam-arena-anotar-configuracion");
        let _ = std::fs::remove_dir_all(&directorio);
        std::fs::create_dir_all(&directorio).expect("arena");

        let rutas = RutasAlmacen::nuevo(directorio.clone());

        assert!(
            cargar_centinela(&rutas)
                .expect("un almacen vacio se lee")
                .configuracion
                .secuencia()
                .is_none(),
            "la arena tenia que empezar sin marca"
        );

        let resultado = anotar_configuracion(&rutas, firmada());

        assert!(
            matches!(resultado, Configuracion::Firmada(_)),
            "anotar no puede cambiar el veredicto de una configuracion buena"
        );
        assert_eq!(
            cargar_centinela(&rutas)
                .expect("releer")
                .configuracion
                .secuencia(),
            Some(valores().secuencia),
            "la marca no subio: la comprobacion de frescura no rechazaria nada jamas"
        );

        let _ = std::fs::remove_dir_all(&directorio);
    }

    /// Y si no se puede anotar, **no se obedece**.
    ///
    /// RPT-078. Obedecer sin dejar constancia deja al proximo arranque sin saber
    /// que se llego a ver esta secuencia, que es exactamente la ventana de
    /// reversion que el paso 5 cierra. La arena es un FICHERO donde el agente
    /// espera un directorio, con lo que la escritura falla sin depender de
    /// permisos ni de que la prueba corra como root.
    #[test]
    fn una_configuracion_que_no_se_puede_anotar_no_se_obedece() {
        let estorbo = std::env::temp_dir().join("eje-latam-arena-anotar-imposible");
        let _ = std::fs::remove_dir_all(&estorbo);
        std::fs::write(&estorbo, b"esto es un fichero, no un directorio").expect("arena");

        let rutas = RutasAlmacen::nuevo(estorbo.join("dentro"));

        assert!(
            matches!(
                anotar_configuracion(&rutas, firmada()),
                Configuracion::NoVerifica(_)
            ),
            "se obedeceria una configuracion cuya secuencia no quedo registrada"
        );

        let _ = std::fs::remove_file(&estorbo);
    }

    /// `respuestaAutomatica` **no puede decir que sí** si cualquiera de las dos
    /// guardas dice que no.
    ///
    /// RPT-081, PA-135. El encargo era atarla al estado de arranque. Informar
    /// sólo eso haría que un sensor de planta —perfil `ot`, almacén impecable—
    /// anunciara `true`: le diría al operador que ese sensor actúa solo cuando
    /// **no lo hará jamás**, porque `evaluar` nunca ejecuta con perfil `ot`.
    ///
    /// Las cuatro combinaciones, porque el fallo que importa es el asimétrico:
    /// una sola guarda cerrada tiene que bastar para cerrar el campo.
    #[test]
    fn la_respuesta_automatica_exige_las_dos_guardas() {
        use guardian_cc::PerfilSegmento::{Corporativo, Ot};
        use guardian_cc::arranque::EstadoArranque;

        let contiene = EstadoArranque::PrimerArranque;
        let no_contiene = EstadoArranque::Supresion {
            secuencia_conocida: 7,
        };

        assert!(
            super::estado_del_agente(Corporativo, &contiene).respuesta_automatica,
            "perfil que la admite y almacen sano: es el unico caso que si"
        );
        assert!(
            !super::estado_del_agente(Ot, &contiene).respuesta_automatica,
            "el perfil OT NUNCA contiene solo (IEC 62443): decir que si es la \
             mentira peligrosa"
        );
        assert!(
            !super::estado_del_agente(Corporativo, &no_contiene).respuesta_automatica,
            "con el inventario suprimido no se contiene, diga lo que diga el perfil"
        );
        assert!(!super::estado_del_agente(Ot, &no_contiene).respuesta_automatica);
    }

    /// Y el resto del estado sale del binario y de la configuración, no de aire.
    #[test]
    fn el_estado_del_agente_declara_su_version_y_su_perfil() {
        use guardian_cc::PerfilSegmento::Ot;
        use guardian_cc::arranque::EstadoArranque;

        let estado = super::estado_del_agente(Ot, &EstadoArranque::PrimerArranque);

        assert_eq!(estado.version, super::VERSION);
        assert_eq!(estado.perfil, eje_ipc::mensajes::PerfilSegmento::Ot);
    }

    /// El perfil no se cruza al pasar al cable.
    ///
    /// RPT-081. El `match` exhaustivo de `perfil_en_el_cable` obliga a traducir
    /// cada perfil; **no impide traducirlo mal**. `Corporativo => Ot` compila
    /// igual de bien, y en el cable significaria que la sala ve una planta donde
    /// hay una oficina — o al reves, que es peor: una oficina donde hay una
    /// planta invita a esperar contencion automatica que nunca va a ocurrir.
    #[test]
    fn el_perfil_no_se_cruza_al_pasar_al_cable() {
        use guardian_cc::PerfilSegmento::{Corporativo, Ot};

        assert_eq!(
            super::perfil_en_el_cable(Corporativo),
            eje_ipc::mensajes::PerfilSegmento::Corporativo
        );
        assert_eq!(
            super::perfil_en_el_cable(Ot),
            eje_ipc::mensajes::PerfilSegmento::Ot
        );
    }

    /// La configuracion firmada NO decide donde vive el almacen.
    ///
    /// RPT-077. La clave con la que se verifica es `<almacen>/clave-cliente.pub`:
    /// si el almacen saliera de la firma, la firma elegiria donde se busca la
    /// clave que decide si creerla. Se comprueba por los dos lados —la ruta sale
    /// de los argumentos, y pasarla con configuracion firmada no aborta— porque
    /// solo uno de los dos dejaria pasar el error.
    #[test]
    fn la_firma_no_puede_mover_el_almacen_donde_vive_su_clave() {
        let argumentos = leidos(&["--almacen", "/tmp/instalacion-de-esta-maquina"]);
        let rutas = rutas_de_instalacion(&argumentos);

        assert_eq!(
            rutas.directorio(),
            std::path::Path::new("/tmp/instalacion-de-esta-maquina")
        );
        assert!(
            rutas
                .clave_operativa()
                .starts_with("/tmp/instalacion-de-esta-maquina"),
            "la clave que verifica la configuracion vive dentro del almacen"
        );
        assert!(
            resolver(argumentos, &firmada()).is_ok(),
            "donde guarda sus ficheros esta maquina es instalacion, no politica firmada"
        );
    }
}

#[cfg(test)]
mod pruebas_opciones {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::{OPCIONES, Opcion, leer_opciones_de, uso};

    fn argumentos(pares: &[&str]) -> Vec<String> {
        pares.iter().map(|texto| (*texto).to_owned()).collect()
    }

    /// Toda opcion anunciada la acepta el analizador.
    ///
    /// RPT-071, PA-122. Es la direccion que un `match` no puede garantizar por
    /// construccion: la puerta de `leer_opciones_de` impide aceptar lo que no
    /// esta en la tabla, pero nada impide **anunciar** algo que el `match`
    /// ignore y que caiga en el brazo de error.
    ///
    /// Un comando documentado que no existe manda a teclear algo que falla, y lo
    /// hara en la sesion en la que menos tiempo hay para averiguar por que. La
    /// misma leccion que PA-119, un piso mas abajo.
    #[test]
    fn toda_opcion_anunciada_se_acepta_de_verdad() {
        for opcion in OPCIONES {
            let Opcion {
                bandera, ejemplo, ..
            } = *opcion;

            // La interfaz es obligatoria: sin ella todo falla por otro motivo y
            // la prueba no diria nada de la opcion que se examina.
            let mut argv = argumentos(&["--interfaz", "lo"]);
            if bandera != "--interfaz" {
                argv.push(bandera.to_owned());
                argv.push(ejemplo.to_owned());
            }

            assert!(
                leer_opciones_de(&argv).is_ok(),
                "'{bandera}' se anuncia en la linea de uso y el analizador la rechaza"
            );
        }
    }

    /// Y ninguna bandera sin anunciar se cuela.
    #[test]
    fn una_bandera_que_no_esta_en_la_tabla_se_rechaza() {
        let argv = argumentos(&["--interfaz", "lo", "--modo-secreto", "si"]);

        assert!(
            leer_opciones_de(&argv).is_err(),
            "el analizador acepto una opcion que la linea de uso no anuncia"
        );
    }

    /// La linea de uso las nombra todas, que es de donde salio el punto.
    #[test]
    fn la_linea_de_uso_nombra_todas_las_opciones() {
        let texto = uso();

        for opcion in OPCIONES {
            assert!(
                texto.contains(opcion.bandera),
                "'{}' no aparece en la linea de uso: nadie podra descubrirla",
                opcion.bandera
            );
        }

        // La que lo destapo, por su nombre: aparecio en RPT-069 al ejecutar el
        // binario sin argumentos en la maquina de pruebas.
        assert!(texto.contains("--directorio-socket"));
    }

    /// Lo obligatorio se distingue de lo opcional en el texto.
    ///
    /// Sin esto, la tabla podria marcar `obligatoria` y la linea presentarlo
    /// entre corchetes, que es como se lee «puedes omitirlo» de algo sin lo cual
    /// el agente no arranca.
    #[test]
    fn lo_obligatorio_no_va_entre_corchetes() {
        let texto = uso();

        for opcion in OPCIONES {
            let entre_corchetes = texto.contains(&format!("[{} ", opcion.bandera));
            assert_eq!(
                entre_corchetes, !opcion.obligatoria,
                "'{}' se presenta al reves de lo que la tabla declara",
                opcion.bandera
            );
        }
    }
}
