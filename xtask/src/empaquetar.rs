//! Empaquetado del sensor headless. RPT-062, PA-107.
//!
//! # Lo que hace y lo que no
//!
//! Produce el árbol del artefacto headless —binario, unidad de servicio,
//! configuración de ejemplo, instalador y guía— y **después lo revisa**.
//!
//! No decide el formato del paquete. `.deb`, `.rpm`, tarball o imagen depende
//! del repositorio firmado, que es infraestructura pendiente (RPT-054 §9). Lo
//! que sí se puede tener hoy es el contenido y las comprobaciones sobre él.
//!
//! # La revisión corre sobre lo producido, no sobre la lista
//!
//! Es la diferencia entera. `eje-manifiesto` no figura en lo que este módulo
//! copia, y eso ya lo garantizaba una prueba desde RPT-025 — que comprueba que
//! `eje-agente` no lo declara como dependencia.
//!
//! Aquel reporte dejó escrito por qué esa prueba **no basta**: nada impide que
//! el empaquetador copie el binario del emisor al instalador. «Sólo lo cierra
//! una comprobación sobre el artefacto, y esa es PA-12 y no existe.»
//!
//! Así que [`revisar`] recorre el árbol **ya escrito** y comprueba lo que hay,
//! no lo que se pretendía poner. Si alguien añade una línea que copie algo de
//! más, la revisión lo ve; si sólo mirara la lista de este fichero, no.
//!
//! # Por qué esto importa más que ninguna otra pieza del paquete
//!
//! Si el emisor de manifiestos viajara en el artefacto, **cada sensor desplegado
//! llevaría encima la capacidad de firmar inventarios**. Un sensor está en un
//! armario de planta, físicamente accesible, y su modelo de amenaza asume que
//! puede caer. Los cinco eslabones de RPT-011 se apoyan en que quien lo
//! comprometa **no pueda firmar**.

use std::fmt::Write as _;
use std::path::Path;

/// Motivo por el que un fichero no puede viajar en el artefacto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hallazgo {
    /// Fichero encontrado, relativo a la raíz del artefacto.
    pub fichero: String,
    /// Por qué no puede estar.
    pub motivo: &'static str,
}

/// Comprueba si un nombre de fichero está prohibido en el artefacto.
///
/// # Por qué por nombre y no por contenido
///
/// Porque el nombre es lo que sobrevive a copiarlo, renombrarlo o comprimirlo, y
/// porque un emisor de manifiestos con otro nombre es un problema distinto —de
/// quien construye el paquete— y no el descuido que esto vigila.
///
/// Se prefiere pasarse: un fichero legítimo que caiga aquí obliga a renombrarlo
/// o a declararlo, y eso es barato. Uno prohibido que se cuele viaja a todos los
/// sensores del cliente.
#[must_use]
pub fn prohibido(nombre: &str) -> Option<&'static str> {
    let minusculas = nombre.to_ascii_lowercase();

    if minusculas.starts_with("eje-manifiesto") {
        return Some(
            "el emisor de manifiestos NO se despliega: daria a cada sensor la \
             capacidad de firmar inventarios (RPT-023, RPT-025)",
        );
    }

    for marca in ["semilla", ".clave", ".priv", "id_ed25519", ".pem"] {
        if minusculas.contains(marca) {
            return Some(
                "material de clave: la firma vive fuera del sensor, que es lo \
                 unico que sostiene la cadena de RPT-011",
            );
        }
    }

    None
}

/// Revisa un árbol ya escrito y devuelve lo que no debería estar.
///
/// # Errores
///
/// Devuelve el motivo si el directorio no se puede recorrer. **No** se degrada a
/// «no encontré nada»: un artefacto que no se puede revisar no es un artefacto
/// limpio, es uno del que no se sabe nada (RPT-006 §4).
pub fn revisar(raiz: &Path) -> Result<Vec<Hallazgo>, String> {
    let mut hallazgos = Vec::new();
    recorrer(raiz, raiz, &mut hallazgos)?;
    hallazgos.sort_by(|uno, otro| uno.fichero.cmp(&otro.fichero));
    Ok(hallazgos)
}

fn recorrer(raiz: &Path, directorio: &Path, hallazgos: &mut Vec<Hallazgo>) -> Result<(), String> {
    let entradas = std::fs::read_dir(directorio)
        .map_err(|error| format!("no se pudo leer {}: {error}", directorio.display()))?;

    for entrada in entradas {
        let entrada = entrada.map_err(|error| format!("entrada ilegible: {error}"))?;
        let ruta = entrada.path();

        if ruta.is_dir() {
            recorrer(raiz, &ruta, hallazgos)?;
            continue;
        }

        let Some(nombre) = ruta.file_name().and_then(|nombre| nombre.to_str()) else {
            // Un nombre que no es UTF-8 no se puede juzgar, y por tanto no se
            // absuelve: se declara.
            hallazgos.push(Hallazgo {
                fichero: ruta.display().to_string(),
                motivo: "nombre de fichero ilegible: no se puede comprobar",
            });
            continue;
        };

        if let Some(motivo) = prohibido(nombre) {
            let relativa = ruta.strip_prefix(raiz).unwrap_or(&ruta);
            hallazgos.push(Hallazgo {
                fichero: relativa.display().to_string(),
                motivo,
            });
        }
    }

    Ok(())
}

/// Unidad de `systemd` del sensor headless.
///
/// # `Restart=always` no es una preferencia
///
/// RPT-054 §7. Desde que el agente late (RPT-053), un proceso que muere y no
/// vuelve deja de latir, y a partir de PA-105 eso es una llamada a la sala. El
/// supervisor tiene que **reiniciar**, no sólo lanzar.
///
/// # Por qué la unidad ya no configura el sensor
///
/// RPT-077, PA-79. Hasta el paso 4b la unidad leía `EnvironmentFile` y pasaba la
/// interfaz, el colector y el grupo por `ExecStart`. Eso significaba que **quien
/// pudiera editar un fichero de texto decidía a qué segmento mira el sensor y a
/// quién informa**, que es exactamente lo que la configuración firmada existe
/// para impedir. Mantener las dos vías habría dejado la firma sin efecto: basta
/// una línea en `ExecStart` para ganarle.
///
/// Lo que queda son las dos cosas que **no** son política del cliente:
///
/// - `--almacen`, porque la clave que verifica la configuración firmada vive
///   dentro del almacén, y una configuración que dijera dónde está el almacén
///   estaría eligiendo dónde se busca la clave que decide si creerla;
/// - `--ciclos 0`, que distingue el servicio de un recorrido de comprobación a
///   mano y no describe al sensor.
///
/// **Un sensor recién instalado no arranca hasta que se le emite configuración.**
/// Es un corte deliberado y el instalador lo dice; la alternativa era dejar en
/// pie el camino que la firma viene a cerrar.
#[must_use]
pub fn unidad_de_servicio() -> String {
    "\
[Unit]
Description=Eje-Agente. Sensor local de Eje-Latam (PremosCorp)
Documentation=https://github.com/Fibela/Eje-Latam
After=network-online.target
Wants=network-online.target

[Service]
Type=simple

# RPT-077, PA-79. Aqui NO se configura el sensor.
#
# La interfaz, el colector, el intervalo de latido, el grupo del socket y la
# identidad en la sala salen de /etc/eje-latam/agente.conf.firmado, que va
# firmado por el administrador del cliente. Pasarlos aqui ademas haria que el
# agente se negara a arrancar: dos sitios donde decidir lo mismo es un sitio
# donde ganarle a la firma.
#
# Queda --almacen porque es instalacion y no politica —la clave que verifica esa
# configuracion vive dentro—, y --ciclos 0 porque distingue el servicio de un
# recorrido de comprobacion a mano.
ExecStart=/usr/local/bin/eje-agente \\
    --almacen /var/lib/eje-latam \\
    --ciclos 0

# RPT-054 §7. Un agente que muere y no vuelve deja de latir, y la sala lo
# leera como un sensor apagado. Reiniciar es parte del producto, no una
# comodidad de operacion.
Restart=always
RestartSec=5

# RPT-067, PA-120. systemd crea /run/eje-latam al arrancar y lo destruye al
# parar. /run es tmpfs: se vacia en cada arranque, asi que el socket huerfano
# —el fichero que sobrevive al proceso y hace que el cliente reciba
# ECONNREFUSED sobre algo que existe— deja de ser posible por construccion.
#
# No se pasa --directorio-socket: el valor de fabrica del agente es este mismo
# directorio, y pasarlo crearia dos sitios donde cambiarlo.
#
# El modo se deja en 0755 a proposito. Cerrarlo mas impediria a la consola
# atravesar el directorio, y quien restringe el acceso es el socket (0660 con el
# grupo que dicta la configuracion firmada), no el camino hasta el.
RuntimeDirectory=eje-latam

# El agente captura tramas. Se le da la capacidad y se le quitan las demas,
# en lugar de correr como root entero.
#
# CAP_CHOWN esta aqui por RPT-069, PA-124, y se observo en maquina real: sin
# ella, asignar al socket el grupo que dicta la configuracion falla con EPERM y el sensor
# se queda SIN ESCUCHA LOCAL. El proceso corre como root, pero root no pertenece
# a ese grupo y cambiar el grupo de un fichero a uno ajeno exige CAP_CHOWN.
#
# Es el precio exacto de PA-82: si el socket no llevara grupo, esta capacidad
# sobraria y la consola necesitaria sudo. Se paga a proposito.
AmbientCapabilities=CAP_NET_RAW CAP_CHOWN
CapabilityBoundingSet=CAP_NET_RAW CAP_CHOWN
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true

# Un solo cometido: la persistencia. Mientras el socket vivio aqui, esta linea
# autorizaba a la vez escribir evidencia y crear el socket, y medir el
# aislamiento no distinguia una cosa de la otra (RPT-067 §3, PA-117).
ReadWritePaths=/var/lib/eje-latam

[Install]
WantedBy=multi-user.target
"
    .to_owned()
}

/// Plantilla de configuración que acompaña al paquete.
///
/// # Esto ya no es la configuración del sensor
///
/// RPT-077, PA-79. Hasta el paso 4b era un `EnvironmentFile` que el sensor leía
/// directamente, y ahí estaba el problema: quien pudiera escribirlo decidía a qué
/// segmento mira y a quién informa. Ahora es **la entrada del emisor**: el
/// administrador se la lleva a su máquina, la rellena, y `eje-manifiesto
/// configurar` produce el fichero firmado que el sensor sí obedece.
///
/// Viaja en el paquete de todas formas porque el técnico que instala es quien
/// sabe cómo se llama la interfaz de esa planta, y tenerla delante evita el viaje
/// de vuelta.
#[must_use]
pub fn configuracion_ejemplo() -> String {
    "\
# Plantilla de configuracion de un sensor. RPT-074, RPT-077.
#
# ESTE FICHERO NO CONFIGURA NADA POR SI SOLO. Es la entrada de:
#
#   eje-manifiesto configurar --semilla <clave> --entrada <este fichero> \\
#       --salida agente.conf.firmado [--anterior <la anterior>]
#
# El resultado firmado se copia a /etc/eje-latam/agente.conf.firmado. El sensor
# obedece ese, y solo ese: los mismos valores puestos en la linea de ordenes le
# impiden arrancar, porque dos sitios donde decidir lo mismo es un sitio donde
# ganarle a la firma.
#
# La clave privada NO vive en el sensor. Este fichero se rellena aqui y se firma
# en la maquina de emision del administrador del cliente.

# Hostname EXACTO del equipo donde esta configuracion sera valida.
#
# No es adorno: sin el, basta copiar la configuracion de un sensor tranquilo
# sobre uno ruidoso y las dos firmas son legitimas. Averigualo con: hostname
maquina = \"planta-3\"

# Identidad de este sensor en la sala. Por ella se correlacionan los sellos y
# por ella se detecta la ausencia de latidos: dos sensores con el mismo nombre
# hacen que el latido de uno tape la muerte del otro.
nombre = \"sensor-planta-3\"

# Interfaz que se vigila. No hay valor por omision a proposito.
interfaz = \"eth0\"

# corporativo | ot
#
# `ot` apaga la Capa B y el descubrimiento activo. Una errata NO cae a
# `corporativo`: se rechaza al emitir.
perfil = \"corporativo\"

# Cada cuanto late el sensor, en milisegundos. Alargarlo alarga la ventana de
# silencio que la sala vigila, y por eso viaja firmado.
intervalo_latido_ms = 30000

# Grupo (numerico) autorizado a consultar por el socket local. PA-82.
# Omitelo para dejar el socket en 0600, accesible solo por el propio agente.
grupo_ipc = 0

# Colector de syslog, host:puerto.
#
# SE ENVIA VACIO A PROPOSITO. Una direccion de ejemplo aqui seria peor que
# ninguna: quien rellene esto sin mirar se llevaria un sensor apuntando a un
# colector que no existe, con `salidaNoDisponible` encendida para siempre.
#
# Vacio es un despliegue valido (RPT-054 §1): el sensor vigila el segmento, lo
# anota en ALM-01 y enciende `sinColector` para que se sepa que nadie fuera
# notara si se apaga.
colector = \"\"
"
    .to_owned()
}

/// Instalador del artefacto.
///
/// # Dice la frase, y por eso existe
///
/// RPT-054 §8.3. La decisión ratificada es «instala aunque no haya colector, y
/// **lo declara a gritos**». Que lo declare el instalador es la mitad que ve la
/// persona que está delante de la máquina; la otra mitad es la condición
/// `sinColector` que ve VIS-04 (RPT-055).
#[must_use]
pub fn instalador() -> String {
    "\
#!/bin/sh
# Instalador del sensor Eje-Agente. RPT-054, PA-107.
set -eu

DESTINO_BIN=${DESTINO_BIN:-/usr/local/bin}
DESTINO_CONF=${DESTINO_CONF:-/etc/eje-latam}
DESTINO_DATOS=${DESTINO_DATOS:-/var/lib/eje-latam}
DESTINO_UNIDAD=${DESTINO_UNIDAD:-/etc/systemd/system}

# --- Integridad, ANTES de tocar el sistema (RPT-073, PA-126) -----------------
#
# Comprueba que el paquete llego entero. NO comprueba que venga de PremosCorp:
# eso es firma de release y esta en PA-14a. Ver el aviso del final.
#
# Falla cerrado sin sha256sum: no poder comprobar no es haber comprobado
# (RPT-006 §4). Un instalador que sigue adelante porque le falta la herramienta
# es exactamente el verde que no afirma nada.
if [ ! -f MANIFIESTO ]; then
    echo \"!! Este paquete no trae MANIFIESTO. No se instala nada.\"
    exit 1
fi

if ! command -v sha256sum >/dev/null 2>&1; then
    echo \"!! No hay sha256sum: no se puede comprobar la integridad del paquete.\"
    echo \"   No se instala nada. Instala coreutils y vuelve a intentarlo.\"
    exit 1
fi

if ! sha256sum -c MANIFIESTO >/dev/null 2>&1; then
    echo \"!! EL PAQUETE NO LLEGO ENTERO.\"
    echo \"\"
    sha256sum -c MANIFIESTO 2>&1 | grep -v ': OK$' || true
    echo \"\"
    echo \"   Una transferencia truncada o un disco inestable lo explican;\"
    echo \"   una manipulacion tambien. No se instala nada.\"
    exit 1
fi

echo \"Integridad: los $(wc -l < MANIFIESTO) ficheros coinciden con su resumen.\"

install -d \"$DESTINO_BIN\" \"$DESTINO_CONF\" \"$DESTINO_DATOS\" \"$DESTINO_UNIDAD\"
install -m 0755 eje-agente \"$DESTINO_BIN/eje-agente\"
install -m 0644 eje-agente.service \"$DESTINO_UNIDAD/eje-agente.service\"

# La plantilla que el ADMINISTRADOR se lleva a su maquina de emision. No es la
# configuracion del sensor: la del sensor va firmada y no se puede escribir aqui.
#
# Se pisa siempre a proposito, porque es un ejemplo y no un ajuste de nadie.
# Lo que NO se toca nunca es agente.conf.firmado: ver mas abajo.
install -m 0644 configuracion-sensor.toml.ejemplo \\
    \"$DESTINO_CONF/configuracion-sensor.toml.ejemplo\"

echo \"\"
echo \"  !! ESTE PAQUETE NO ESTA FIRMADO.\"
echo \"     Se comprobo que llego entero, NO que venga de PremosCorp.\"
echo \"     Quien pueda sustituir el paquete puede recalcular los resumenes.\"
echo \"     La firma de release es PA-14a y exige custodia en hardware.\"
echo \"\"

# RPT-077, PA-79. Un sensor sin configuracion firmada NO arranca, y decirlo aqui
# es la mitad que ve la persona que esta delante de la maquina. La otra mitad es
# lo que dice el propio agente al intentar arrancar.
#
# Se comprueba la ausencia, no se instala nada: esta es la unica configuracion
# que este guion no puede producir, porque hace falta la clave del cliente y esa
# no vive en el sensor.
if [ -f \"$DESTINO_CONF/agente.conf.firmado\" ]; then
    echo \"Instalado. Este sensor ya tiene configuracion firmada; se ha respetado.\"
    echo \"\"
    echo \"Despues: systemctl restart eje-agente\"
else
    echo \"  !! ESTE SENSOR TODAVIA NO VIGILA NADA.\"
    echo \"     No hay $DESTINO_CONF/agente.conf.firmado, y sin ella el agente\"
    echo \"     no sabe que interfaz mirar ni a que sala informar. Arrancarlo\"
    echo \"     ahora no da un sensor a medias: da uno que lo declara y espera.\"
    echo \"\"
    echo \"     Desde la maquina de emision del administrador del cliente:\"
    echo \"\"
    echo \"       1. copia $DESTINO_CONF/configuracion-sensor.toml.ejemplo\"
    echo \"          y ponle la interfaz, el colector y el hostname DE ESTA maquina\"
    echo \"       2. eje-manifiesto configurar --semilla <clave> \\\\\"
    echo \"            --entrada <toml> --salida agente.conf.firmado \\\\\"
    echo \"            [--anterior <la anterior, si la hay>]\"
    echo \"       3. traelo a $DESTINO_CONF/agente.conf.firmado\"
    echo \"\"
    echo \"     El campo 'maquina' tiene que ser: $(hostname 2>/dev/null || echo '<hostname>')\"
    echo \"     Una configuracion emitida para otro equipo NO la acepta este.\"
    echo \"\"
    echo \"Despues: systemctl enable --now eje-agente\"
fi
"
    .to_owned()
}

/// Construye el árbol del artefacto headless y lo revisa.
///
/// # Se exige el binario de `release`
///
/// Empaquetar el de depuración y llamarlo artefacto sería mentir sobre lo que
/// es: otro binario, otro tamaño, otras garantías. Se falla cerrado con el
/// comando que falta.
///
/// Devuelve los ficheros escritos, relativos al destino.
///
/// # Errores
///
/// Falta el binario, el destino no se puede escribir, o **la revisión encuentra
/// algo prohibido en lo recién producido**.
pub fn empaquetar(raiz_repo: &Path, destino: &Path) -> Result<Vec<String>, String> {
    let binario = raiz_repo.join("target/release/eje-agente");
    if !binario.is_file() {
        return Err(format!(
            "falta {}\n\
             Empaquetar el binario de depuracion y llamarlo artefacto seria mentir \
             sobre lo que es.\n\
             Ejecuta: cargo build --release -p eje-agente",
            binario.display()
        ));
    }

    // Un destino que ya existe se vacia: un artefacto con restos de otra
    // ejecucion es exactamente lo que la revision no puede distinguir de un
    // artefacto correcto.
    let _ = std::fs::remove_dir_all(destino);
    std::fs::create_dir_all(destino)
        .map_err(|error| format!("no se pudo crear {}: {error}", destino.display()))?;

    let escribir = |nombre: &str, contenido: &str| -> Result<String, String> {
        let ruta = destino.join(nombre);
        std::fs::write(&ruta, contenido)
            .map_err(|error| format!("no se pudo escribir {}: {error}", ruta.display()))?;
        Ok(nombre.to_owned())
    };

    let mut ficheros = vec![escribir("eje-agente.service", &unidad_de_servicio())?];
    ficheros.push(escribir(
        "configuracion-sensor.toml.ejemplo",
        &configuracion_ejemplo(),
    )?);
    ficheros.push(escribir("instalar.sh", &instalador())?);

    std::fs::copy(&binario, destino.join("eje-agente"))
        .map_err(|error| format!("no se pudo copiar el binario: {error}"))?;
    ficheros.push("eje-agente".to_owned());
    ficheros.sort();

    // El manifiesto se escribe ANTES de revisar, para que la revision lo mire
    // como mira todo lo demas. Se deriva del disco, igual que `revisar`: una
    // lista construida a partir de `ficheros` diria lo que el empaquetador creia
    // haber escrito, que es justo lo que PA-107 dejo de admitir.
    let resumenes = manifiesto(destino)?;
    std::fs::write(destino.join(NOMBRE_MANIFIESTO), &resumenes)
        .map_err(|error| format!("no se pudo escribir el manifiesto: {error}"))?;
    ficheros.push(NOMBRE_MANIFIESTO.to_owned());
    ficheros.sort();

    // Y AHORA se revisa lo que hay en el disco. No la lista de arriba: el
    // artefacto.
    let hallazgos = revisar(destino)?;
    if !hallazgos.is_empty() {
        let detalle: Vec<String> = hallazgos
            .iter()
            .map(|hallazgo| format!("  {} — {}", hallazgo.fichero, hallazgo.motivo))
            .collect();
        return Err(format!(
            "el artefacto contiene {} fichero(s) que no pueden desplegarse:\n{}",
            hallazgos.len(),
            detalle.join("\n")
        ));
    }

    let tarro = empaquetar_tar(destino, &ficheros)?;
    ficheros.push(format!(
        "-> {}",
        tarro.file_name().map_or_else(
            || tarro.display().to_string(),
            |n| n.to_string_lossy().into()
        )
    ));

    Ok(ficheros)
}

/// Nombre del manifiesto de resumenes dentro del artefacto.
pub const NOMBRE_MANIFIESTO: &str = "MANIFIESTO";

/// Manifiesto de integridad del artefacto, leido **del disco**.
///
/// RPT-073, PA-126.
///
/// # Formato prestado a proposito
///
/// `<resumen>  <nombre>`, que es exactamente lo que `sha256sum -c` come. Un
/// formato propio obligaria al instalador a implementar un analizador en `sh`, y
/// un analizador en `sh` dentro del guion que decide si se instala es la ultima
/// pieza que este proyecto quiere escribir.
///
/// # Lo que afirma y lo que NO
///
/// Afirma **integridad**: que el paquete llego entero. Una transferencia
/// truncada, un `scp` a medias o un disco inestable se detectan aqui.
///
/// **No afirma autenticidad.** Quien pueda sustituir el artefacto puede
/// recalcular los resumenes. Eso exige firma de release, que es PA-14a y esta en
/// rojo: la clave de codigo vive en hardware que aun no existe, y
/// `DominioClave::PremosCorp` —cuya documentacion dice «firma binarios, reglas e
/// imagenes de release»— no tiene todavia sitio donde vivir en `RutasAlmacen`.
///
/// El hueco se declara a gritos en el instalador, como el colector vacio de
/// RPT-054 §4.1. Un paquete que dijera «verificado» habiendo comprobado solo
/// resumenes seria peor que uno que no comprueba nada.
///
/// # Errores
///
/// `Err` si el directorio no se puede recorrer o un fichero no se puede leer. No
/// se degrada a un manifiesto parcial: un manifiesto al que le falta una linea
/// declara integro un paquete que nadie miro.
pub fn manifiesto(destino: &Path) -> Result<String, String> {
    let entradas = std::fs::read_dir(destino)
        .map_err(|error| format!("no se pudo leer {}: {error}", destino.display()))?;

    let mut lineas: Vec<(String, String)> = Vec::new();

    for entrada in entradas {
        let entrada = entrada
            .map_err(|error| format!("entrada ilegible en {}: {error}", destino.display()))?;
        let ruta = entrada.path();

        if !ruta.is_file() {
            continue;
        }

        let nombre = entrada.file_name().to_string_lossy().into_owned();
        // El manifiesto no se resume a si mismo: al escribirlo cambiaria su
        // propio contenido y ninguna comprobacion cuadraria jamas.
        if nombre == NOMBRE_MANIFIESTO {
            continue;
        }

        let bytes = std::fs::read(&ruta)
            .map_err(|error| format!("no se pudo leer {}: {error}", ruta.display()))?;

        lineas.push((nombre, crate::vectores::resumir(&bytes)));
    }

    // Orden estable: el manifiesto de dos empaquetados del mismo arbol tiene que
    // ser byte a byte identico, o la firma de PA-14a cambiaria sin que cambie
    // nada.
    lineas.sort();

    let mut texto = String::new();
    for (nombre, resumen) in &lineas {
        // `writeln!` sobre un `String` no puede fallar.
        let _ = writeln!(texto, "{resumen}  {nombre}");
    }

    Ok(texto)
}

/// Empaqueta el artefacto en un `.tar.gz` **reproducible**.
///
/// RPT-073, PA-126.
///
/// # Por que las cabeceras se escriben a mano
///
/// `tar` guarda fecha, usuario y grupo. Con los del sistema, dos empaquetados
/// del mismo arbol producen ficheros distintos, y cuando llegue la firma de
/// PA-14a la firma cambiaria sin que cambie nada de lo firmado. Se fijan a cero:
/// el contenido es lo unico que decide los bytes de salida.
///
/// # Errores
///
/// `Err` si algun fichero no se puede leer o el destino no se puede escribir.
fn empaquetar_tar(destino: &Path, ficheros: &[String]) -> Result<std::path::PathBuf, String> {
    let ruta = destino.with_extension("tar.gz");

    let salida = std::fs::File::create(&ruta)
        .map_err(|error| format!("no se pudo crear {}: {error}", ruta.display()))?;

    // Sin marca de tiempo en la cabecera gzip, por lo mismo que en las de tar.
    let comprimido = flate2::write::GzEncoder::new(salida, flate2::Compression::default());
    let mut constructor = tar::Builder::new(comprimido);

    for nombre in ficheros {
        // Las lineas decorativas de la lista no son ficheros.
        if nombre.starts_with("-> ") {
            continue;
        }

        let origen = destino.join(nombre);
        let bytes = std::fs::read(&origen)
            .map_err(|error| format!("no se pudo leer {}: {error}", origen.display()))?;

        // Ejecutable solo lo que tiene que ejecutarse. El instalador vuelve a
        // imponer los modos con `install -m`, asi que esto es lo que ve quien
        // desempaqueta a mano.
        let modo = if nombre == "eje-agente" || nombre == "instalar.sh" {
            0o755
        } else {
            0o644
        };

        let mut cabecera = tar::Header::new_gnu();
        cabecera.set_size(bytes.len() as u64);
        cabecera.set_mode(modo);
        cabecera.set_mtime(0);
        cabecera.set_uid(0);
        cabecera.set_gid(0);
        cabecera.set_cksum();

        constructor
            .append_data(&mut cabecera, format!("eje-agente/{nombre}"), &bytes[..])
            .map_err(|error| format!("no se pudo anadir {nombre} al paquete: {error}"))?;
    }

    constructor
        .into_inner()
        .map_err(|error| format!("no se pudo cerrar el paquete: {error}"))?
        .finish()
        .map_err(|error| format!("no se pudo comprimir el paquete: {error}"))?;

    Ok(ruta)
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Directorio de prueba que se limpia al soltarse.
    struct Directorio(std::path::PathBuf);

    impl Directorio {
        fn nuevo(nombre: &str) -> Self {
            let ruta = std::env::temp_dir().join(format!("eje-latam-paquete-{nombre}"));
            let _ = std::fs::remove_dir_all(&ruta);
            std::fs::create_dir_all(&ruta).expect("directorio de prueba");
            Self(ruta)
        }
    }

    impl Drop for Directorio {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn el_emisor_de_manifiestos_no_puede_viajar_en_el_artefacto() {
        // La regla que sostiene los cinco eslabones de RPT-011. Si viajara, cada
        // sensor desplegado llevaria encima la capacidad de firmar inventarios.
        assert!(prohibido("eje-manifiesto").is_some());
        assert!(prohibido("eje-manifiesto.exe").is_some());
        assert!(
            prohibido("EJE-MANIFIESTO").is_some(),
            "ni cambiando la caja"
        );
    }

    #[test]
    fn tampoco_el_material_de_clave() {
        for nombre in [
            "semilla.bin",
            "cliente.clave",
            "firma.priv",
            "id_ed25519",
            "certificado.pem",
        ] {
            assert!(prohibido(nombre).is_some(), "{nombre}");
        }
    }

    #[test]
    fn lo_que_si_va_no_se_marca() {
        for nombre in [
            "eje-agente",
            "eje-agente.service",
            "configuracion-sensor.toml.ejemplo",
            "instalar.sh",
            "LEEME.md",
        ] {
            assert_eq!(prohibido(nombre), None, "{nombre}");
        }
    }

    #[test]
    fn la_revision_mira_el_disco_y_no_la_lista_del_empaquetador() {
        // ESTA es la prueba que da sentido al modulo. RPT-025 §61 dejo escrito
        // que la comprobacion sobre el `Cargo.toml` es necesaria y **no
        // suficiente**: nada impide que el empaquetador copie el binario del
        // emisor. Aqui el fichero no lo puso `empaquetar`, y aun asi aparece.
        let directorio = Directorio::nuevo("revision-en-disco");
        std::fs::write(directorio.0.join("eje-agente"), b"binario").expect("escribir");
        std::fs::write(directorio.0.join("eje-manifiesto"), b"colado").expect("escribir");

        let hallazgos = revisar(&directorio.0).expect("se puede recorrer");

        assert_eq!(hallazgos.len(), 1);
        assert_eq!(hallazgos[0].fichero, "eje-manifiesto");
    }

    #[test]
    fn tambien_lo_escondido_en_un_subdirectorio() {
        let directorio = Directorio::nuevo("revision-honda");
        let honda = directorio.0.join("extras").join("herramientas");
        std::fs::create_dir_all(&honda).expect("crear");
        std::fs::write(honda.join("semilla.bin"), b"x").expect("escribir");

        let hallazgos = revisar(&directorio.0).expect("se puede recorrer");
        assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
        assert!(hallazgos[0].fichero.contains("semilla.bin"));
    }

    #[test]
    fn un_arbol_limpio_no_inventa_hallazgos() {
        let directorio = Directorio::nuevo("revision-limpia");
        std::fs::write(directorio.0.join("eje-agente"), b"binario").expect("escribir");
        std::fs::write(directorio.0.join("instalar.sh"), b"#!/bin/sh").expect("escribir");

        assert!(
            revisar(&directorio.0)
                .expect("se puede recorrer")
                .is_empty()
        );
    }

    #[test]
    fn un_arbol_que_no_se_puede_recorrer_no_se_declara_limpio() {
        // «No se pudo comprobar» no es «no habia nada». RPT-006 §4.
        let inexistente = std::env::temp_dir().join("eje-latam-paquete-inexistente-jamas");
        let _ = std::fs::remove_dir_all(&inexistente);

        assert!(revisar(&inexistente).is_err());
    }

    #[test]
    fn la_unidad_reinicia_y_no_solo_arranca() {
        // RPT-054 §7. Desde que el agente late, un proceso que muere y no vuelve
        // es un sensor que la sala da por apagado.
        let unidad = unidad_de_servicio();

        assert!(unidad.contains("Restart=always"), "{unidad}");
        assert!(unidad.contains("RestartSec="), "{unidad}");
    }

    #[test]
    fn la_unidad_no_pide_root_entero() {
        // El agente necesita capturar. Se le da `CAP_NET_RAW` y se le quitan las
        // demas, que es lo que RPT-051 §1 daba por supuesto sin que nadie lo
        // hubiera escrito en ninguna parte.
        let unidad = unidad_de_servicio();

        assert!(unidad.contains("AmbientCapabilities=CAP_NET_RAW"));
        assert!(unidad.contains("NoNewPrivileges=true"));
    }

    /// El manifiesto cubre **todo** lo que viaja, y sale del disco.
    ///
    /// RPT-073, PA-126. Se deriva del arbol escrito, no de la lista que el
    /// empaquetador creyo escribir: es la misma leccion de PA-107, donde la
    /// revision se movio de la lista al disco.
    #[test]
    fn el_manifiesto_cubre_todo_lo_que_viaja_y_no_se_resume_a_si_mismo() {
        let arena = std::env::temp_dir().join("eje-manifiesto-cobertura");
        let _ = std::fs::remove_dir_all(&arena);
        std::fs::create_dir_all(&arena).expect("crea la arena");

        std::fs::write(arena.join("eje-agente"), b"binario").expect("escribe");
        std::fs::write(arena.join("instalar.sh"), b"guion").expect("escribe");
        std::fs::write(arena.join(NOMBRE_MANIFIESTO), b"de una vuelta anterior").expect("escribe");

        let texto = manifiesto(&arena).expect("se puede leer la arena");
        let nombres: Vec<&str> = texto
            .lines()
            .filter_map(|linea| linea.split_whitespace().nth(1))
            .collect();

        assert_eq!(
            nombres,
            vec!["eje-agente", "instalar.sh"],
            "el manifiesto debe cubrir lo que viaja y excluirse a si mismo: {texto}"
        );

        // El resumen es el de los bytes de verdad, no un hueco con la forma de un
        // resumen.
        assert!(
            texto.contains(&crate::vectores::resumir(b"binario")),
            "el resumen no corresponde al contenido: {texto}"
        );

        let _ = std::fs::remove_dir_all(&arena);
    }

    /// Dos empaquetados del mismo arbol dan el mismo manifiesto.
    ///
    /// Cuando llegue la firma de PA-14a, un manifiesto que cambiara por el orden
    /// del sistema de ficheros haria cambiar la firma sin que cambie nada de lo
    /// firmado.
    #[test]
    fn el_manifiesto_es_estable() {
        let arena = std::env::temp_dir().join("eje-manifiesto-estable");
        let _ = std::fs::remove_dir_all(&arena);
        std::fs::create_dir_all(&arena).expect("crea la arena");

        for nombre in ["zeta", "alfa", "media"] {
            std::fs::write(arena.join(nombre), nombre.as_bytes()).expect("escribe");
        }

        let primero = manifiesto(&arena).expect("lee");
        let segundo = manifiesto(&arena).expect("lee");

        assert_eq!(primero, segundo);
        assert!(
            primero.lines().count() == 3 && primero.contains("  alfa\n"),
            "el manifiesto debe estar ordenado y completo: {primero}"
        );
        assert!(
            primero.find("  alfa").unwrap_or(usize::MAX) < primero.find("  zeta").unwrap_or(0),
            "el orden tiene que ser estable, no el del sistema de ficheros"
        );

        let _ = std::fs::remove_dir_all(&arena);
    }

    /// Dos empaquetados del mismo arbol producen el **mismo** fichero.
    ///
    /// RPT-073 §6. Es la afirmacion que sostiene la firma de PA-14a: si `tar`
    /// guardara la fecha y el usuario del sistema, la firma cambiaria sin que
    /// cambie nada de lo firmado, y nadie podria distinguir un paquete
    /// reempaquetado de uno alterado.
    ///
    /// Sin esta prueba la reproducibilidad seria una afirmacion del reporte y
    /// nada mas.
    #[test]
    fn el_paquete_es_reproducible() {
        let arena = std::env::temp_dir().join("eje-paquete-reproducible");
        let _ = std::fs::remove_dir_all(&arena);

        let armar = |sufijo: &str| -> Vec<u8> {
            let destino = arena.join(sufijo);
            std::fs::create_dir_all(&destino).expect("crea la arena");
            std::fs::write(destino.join("eje-agente"), b"binario").expect("escribe");
            std::fs::write(destino.join("instalar.sh"), b"guion").expect("escribe");

            let ficheros = vec!["eje-agente".to_owned(), "instalar.sh".to_owned()];
            let tarro = empaquetar_tar(&destino, &ficheros).expect("empaqueta");
            std::fs::read(&tarro).expect("lee el paquete")
        };

        let primero = armar("uno");
        // Un instante despues, y en otro directorio: si la fecha o la ruta se
        // colaran en las cabeceras, aqui se veria.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let segundo = armar("dos");

        assert_eq!(
            primero, segundo,
            "el paquete cambia entre dos empaquetados del mismo contenido"
        );

        let _ = std::fs::remove_dir_all(&arena);
    }

    /// La comprobacion de integridad va **antes** de tocar el sistema.
    ///
    /// Comprobar despues de copiar deja media instalacion hecha con ficheros que
    /// no son los que se enviaron, que es peor que no comprobar.
    #[test]
    fn el_instalador_comprueba_la_integridad_antes_de_instalar() {
        let guion = instalador();

        let comprobacion = guion
            .find("sha256sum -c MANIFIESTO")
            .expect("el instalador debe comprobar el manifiesto");
        let primera_escritura = guion
            .find("install -d")
            .expect("el instalador debe crear los destinos");

        assert!(
            comprobacion < primera_escritura,
            "la integridad se comprueba despues de empezar a instalar"
        );
    }

    /// Sin la herramienta no se instala: no poder comprobar no es haber
    /// comprobado (RPT-006 §4).
    #[test]
    fn sin_sha256sum_el_instalador_falla_cerrado() {
        let guion = instalador();

        assert!(guion.contains("command -v sha256sum"));
        assert!(
            guion.contains("No se instala nada"),
            "sin herramienta el guion debe negarse, no seguir"
        );
    }

    /// Y dice a gritos lo que **no** ha comprobado.
    ///
    /// RPT-073. Un paquete que dijera «verificado» habiendo mirado solo resumenes
    /// seria peor que uno que no mira nada: el operador creeria tener una cadena
    /// de confianza que no tiene.
    #[test]
    fn el_instalador_declara_que_el_paquete_no_esta_firmado() {
        let guion = instalador();

        assert!(guion.contains("NO ESTA FIRMADO"));
        assert!(
            guion.contains("PA-14a"),
            "el aviso debe decir de que depende que deje de ser cierto"
        );
    }

    /// La unidad conserva la capacidad que el socket con grupo necesita.
    ///
    /// RPT-069, PA-124. **Observado en máquina real**: con
    /// `CapabilityBoundingSet=CAP_NET_RAW` a secas, asignar al socket el grupo de
    /// `EJE_GRUPO_IPC` falla con `EPERM` y el sensor arranca **sin escucha
    /// local**. La consola no puede conectarse a un sensor instalado.
    ///
    /// Los dos mecanismos eran correctos por separado —endurecer la unidad, y
    /// restringir el socket a un grupo— y juntos se anulaban. Ninguna prueba de
    /// texto podía verlo: hacía falta un `systemd` que aplicara el conjunto
    /// acotado de verdad.
    ///
    /// # Por qué se comprueban las dos directivas
    ///
    /// `AmbientCapabilities` la concede; `CapabilityBoundingSet` pone el techo.
    /// Conceder una capacidad por encima del techo no la otorga, así que una
    /// sola de las dos en verde no afirma nada.
    #[test]
    fn la_unidad_conserva_la_capacidad_de_asignar_el_grupo_del_socket() {
        let unidad = unidad_de_servicio();

        for directiva in ["AmbientCapabilities", "CapabilityBoundingSet"] {
            let declarado = unidad
                .lines()
                .find_map(|linea| linea.strip_prefix(&format!("{directiva}=")))
                .unwrap_or_else(|| panic!("la unidad debe declarar {directiva}"));

            assert!(
                declarado.split_whitespace().any(|cap| cap == "CAP_CHOWN"),
                "{directiva} sin CAP_CHOWN: el socket se quedaria sin grupo y la consola sin sensor ({declarado})"
            );
            assert!(
                declarado.split_whitespace().any(|cap| cap == "CAP_NET_RAW"),
                "{directiva} sin CAP_NET_RAW: el agente no podria capturar ({declarado})"
            );
        }
    }

    /// El directorio que `systemd` crea es el que el agente usa de fabrica.
    ///
    /// RPT-067, PA-120. `RuntimeDirectory=X` significa `/run/X`, y esa regla es
    /// de `systemd`, no nuestra. Que coincida con el valor de fabrica del agente
    /// es una afirmacion que hay que comprobar: si alguien cambia uno de los dos,
    /// el servicio arranca, no encuentra el directorio y la consola se queda sin
    /// nadie al otro lado, sin que ninguna prueba se entere.
    #[test]
    fn el_directorio_volatil_de_la_unidad_es_el_que_el_agente_espera() {
        let unidad = unidad_de_servicio();

        let declarado = unidad
            .lines()
            .find_map(|linea| linea.strip_prefix("RuntimeDirectory="))
            .expect("la unidad debe declarar RuntimeDirectory");

        assert_eq!(
            format!("/run/{declarado}"),
            guardian_cc::arranque::DIRECTORIO_SOCKET_POR_OMISION,
            "systemd crearia /run/{declarado} y el agente abriria el socket en otro sitio"
        );
    }

    /// Y el punto de encuentro que declara el contrato es el que el agente abre.
    ///
    /// RPT-079 §2.1, PA-132. La prueba de arriba ata la unidad al agente; ésta
    /// ata el **contrato** al agente, que es la mitad que faltaba y por donde se
    /// coló el defecto: la consola declaraba su ruta por su cuenta, se quedó en
    /// `/run/eje` cuando RPT-067 movió el socket, y **con los valores de fábrica
    /// un sensor sano y una consola sana no se encontraban**.
    ///
    /// Nadie lo vio porque los guiones de desarrollo pasan `EJE_SOCKET` a mano.
    /// El defecto sólo aparecía en un despliegue de verdad, que es donde no hay
    /// nadie mirando.
    ///
    /// La comprobación del lado TypeScript vive en `contrato.prueba.ts`, por lo
    /// mismo que las de los canales: media barrera no es una barrera.
    #[test]
    fn el_punto_de_encuentro_del_contrato_es_el_que_el_agente_abre() {
        let manifiesto = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("contrato-ipc.toml"),
        )
        .expect("contrato-ipc.toml es la fuente de verdad del puente");

        // Se lee el valor que sigue a `[socket]` y no el primero que aparezca:
        // el manifiesto tiene `nombre =` en cada canal y en cada campo, y un
        // `find` ingenuo se llevaria el de `obtener-estado-agente`.
        let seccion = manifiesto
            .split("[socket]")
            .nth(1)
            .expect("el contrato debe declarar el punto de encuentro");

        let valor_de = |clave: &str| -> String {
            seccion
                .lines()
                .take_while(|linea| !linea.trim_start().starts_with('['))
                .find_map(|linea| linea.trim().strip_prefix(clave)?.split('"').nth(1))
                .unwrap_or_else(|| panic!("[socket] no declara {clave}"))
                .to_owned()
        };

        let directorio = valor_de("directorio =");
        let nombre = valor_de("nombre =");

        assert_eq!(
            directorio,
            guardian_cc::arranque::DIRECTORIO_SOCKET_POR_OMISION,
            "el contrato manda a la consola a un directorio y el agente abre otro"
        );

        let rutas = guardian_cc::arranque::RutasAlmacen::nuevo(std::path::PathBuf::from("/datos"));
        assert_eq!(
            rutas.socket(),
            std::path::Path::new(&directorio).join(&nombre),
            "el contrato y el agente no componen la misma ruta"
        );
    }

    /// La orden de arranque que la unidad entrega al nucleo, sin comentarios.
    ///
    /// # Por que no vale mirar el fichero entero
    ///
    /// La primera version de la prueba de abajo hacia
    /// `unidad.contains("--directorio-socket")` sobre todo el texto, y **fallo**
    /// — no porque `ExecStart` pasara el argumento, sino porque un comentario
    /// explica que no se pasa. La prueba se acusaba a si misma.
    ///
    /// Es la misma familia que llevo a la caja de arena del instalador y a la
    /// cobertura: **toda comprobacion que lee texto tiene que quedarse con la
    /// parte que decide**. Aqui la parte que decide son los argumentos, no la
    /// prosa que los rodea.
    fn orden_de_arranque(unidad: &str) -> String {
        let mut orden = String::new();
        let mut dentro = false;

        for linea in unidad.lines() {
            if linea.starts_with("ExecStart=") {
                dentro = true;
            } else if !dentro {
                continue;
            }

            let continua = linea.trim_end().ends_with('\\');
            orden.push_str(linea.trim_end().trim_end_matches('\\'));
            orden.push(' ');

            if !continua {
                break;
            }
        }

        orden
    }

    /// Y la unidad **no** pasa `--directorio-socket`.
    ///
    /// Pasarlo crearia dos sitios donde cambiar la misma cosa, y la prueba de
    /// arriba dejaria de significar nada: comprobaria que dos valores coinciden
    /// mientras un tercero, el del argumento, manda de verdad.
    #[test]
    fn la_unidad_no_repite_el_directorio_del_socket() {
        let arranque = orden_de_arranque(&unidad_de_servicio());

        // El ancla contra un extractor roto. Era `--interfaz` hasta RPT-077, que
        // se lo llevo a la configuracion firmada; ahora es `--almacen`, que es
        // instalacion y se queda. El ancla tiene que ser algo que la orden lleve
        // de verdad, o la negacion de abajo pasa sin comprobar nada.
        assert!(
            arranque.contains("--almacen"),
            "la orden de arranque no se extrajo bien: {arranque}"
        );
        assert!(
            !arranque.contains("--directorio-socket"),
            "ExecStart dicta el directorio del socket y deberia dejarselo al valor de fabrica: {arranque}"
        );
    }

    /// Lo persistente y lo volatil no comparten autorizacion de escritura.
    #[test]
    fn readwritepaths_cubre_la_persistencia_y_nada_mas() {
        let unidad = unidad_de_servicio();

        let rutas: Vec<&str> = unidad
            .lines()
            .filter_map(|linea| linea.strip_prefix("ReadWritePaths="))
            .collect();

        assert_eq!(
            rutas,
            vec!["/var/lib/eje-latam"],
            "ReadWritePaths debe autorizar solo el directorio de datos"
        );
    }

    /// La unidad no configura el sensor. RPT-077, PA-79.
    ///
    /// # Por que se mira `ExecStart` y no el fichero entero
    ///
    /// Mismo motivo que en `la_unidad_no_repite_el_directorio_del_socket`: los
    /// comentarios de la unidad **nombran** los parametros para explicar por que
    /// no estan, y una prueba sobre el texto completo se acusaria a si misma. La
    /// leccion es de PA-129, un piso mas arriba.
    #[test]
    fn la_unidad_no_configura_el_sensor() {
        let unidad = unidad_de_servicio();
        let arranque = orden_de_arranque(&unidad);

        // Sin esto, un extractor roto haria vacuas las negaciones de abajo.
        assert!(
            arranque.contains("--ciclos"),
            "la orden de arranque no se extrajo bien: {arranque}"
        );

        for dictado in [
            "--interfaz",
            "--syslog",
            "--grupo-ipc",
            "--intervalo-latido",
            "--nombre",
            "--perfil",
        ] {
            assert!(
                !arranque.contains(dictado),
                "'{dictado}' lo dicta la configuracion firmada, y pasarlo aqui \
                 impide arrancar al agente: {arranque}"
            );
        }

        assert!(
            !unidad.contains("EnvironmentFile"),
            "mientras la unidad lea un fichero de entorno, quien pueda editarlo \
             decide a que segmento mira el sensor y la firma no vale nada: {unidad}"
        );
    }

    /// Y el instalador dice que sin configuracion firmada esto no vigila nada.
    ///
    /// RPT-054 §4.1 se ratifico como «instala y lo declara a gritos». El grito
    /// cambia de asunto en RPT-077 —antes era el colector ausente, ahora es la
    /// configuracion ausente, que es mas grande y lo contiene— pero la mitad que
    /// ve la persona delante de la maquina tiene que seguir existiendo.
    #[test]
    fn el_instalador_dice_que_sin_configuracion_firmada_no_se_vigila_nada() {
        let guion = instalador();

        assert!(guion.contains("TODAVIA NO VIGILA NADA"), "{guion}");
        assert!(guion.contains("eje-manifiesto configurar"), "{guion}");
        assert!(guion.contains("agente.conf.firmado"), "{guion}");
    }

    /// Y NO produce esa configuracion: no puede, y fingirlo seria peor.
    ///
    /// La clave con la que se firma no vive en el sensor. Un instalador que
    /// escribiera un `agente.conf.firmado` cualquiera dejaria un fichero que no
    /// verifica, y el agente lo leeria como manipulacion.
    #[test]
    fn el_instalador_no_fabrica_una_configuracion_firmada() {
        let guion = instalador();

        assert!(
            !guion.contains("install -m 0640 agente.conf.firmado")
                && !guion.contains("> \\\"$DESTINO_CONF/agente.conf.firmado\\\""),
            "el instalador no puede firmar nada: la clave del cliente no esta aqui"
        );
    }

    #[test]
    fn la_plantilla_no_trae_un_colector_inventado() {
        // Una direccion de ejemplo seria peor que ninguna: quien rellene la
        // plantilla sin mirar se llevaria un sensor firmado apuntando a un
        // colector inexistente, con `salidaNoDisponible` encendida para siempre.
        let ejemplo = configuracion_ejemplo();

        assert!(ejemplo.contains("colector = \"\""), "{ejemplo}");
        assert!(
            !ejemplo.contains("colector = \"127."),
            "un colector de ejemplo se despliega tal cual: {ejemplo}"
        );
    }

    /// La plantilla la lee de verdad el emisor.
    ///
    /// RPT-077. Es la prueba que impide que este fichero se convierta en prosa:
    /// mientras solo se comprobara que contiene ciertas palabras, una plantilla
    /// con un campo mal escrito viajaria en cada paquete y fallaria en la planta,
    /// delante del tecnico, el dia de la instalacion.
    ///
    /// Se quitan las lineas de comentario porque el ejemplo lleva dentro un
    /// bloque `eje-manifiesto configurar ...` que no es TOML — la misma familia
    /// de PA-129: quedarse con la parte que decide.
    #[test]
    fn la_plantilla_la_acepta_el_emisor_de_verdad() {
        let ejemplo = configuracion_ejemplo();
        let toml: String = ejemplo
            .lines()
            .filter(|linea| !linea.trim_start().starts_with('#'))
            .collect::<Vec<&str>>()
            .join("\n");

        let entrada = eje_manifiesto::entrada::ConfiguracionEntrada::analizar(&toml)
            .expect("la plantilla que viaja en el paquete tiene que analizarse");

        entrada
            .valores(1)
            .expect("y tiene que producir valores firmables");
    }

    #[test]
    fn sin_binario_de_release_no_se_empaqueta_nada() {
        // Empaquetar el de depuracion y llamarlo artefacto seria mentir sobre lo
        // que es. Se falla cerrado, con el comando que falta.
        let vacio = Directorio::nuevo("sin-release");
        let destino = Directorio::nuevo("destino-sin-release");

        let error = empaquetar(&vacio.0, &destino.0).expect_err("no hay binario");
        assert!(error.contains("cargo build --release"), "{error}");
    }
}
