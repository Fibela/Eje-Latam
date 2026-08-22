//! Caja de arena del instalador. RPT-063, PA-116.
//!
//! # Por qué no es un guion de shell
//!
//! `instalar.sh` **tiene** que ser shell: corre en la máquina del cliente. El
//! arnés que lo comprueba, no. Un `test-instalador.sh` sería un guion que
//! verifica cosas y que nadie verifica, y `xtask` existe exactamente para eso
//! (RPT-003 §9.5, PA-11): «punto de entrada único para las verificaciones
//! propias del proyecto, **en sustitución de scripts de shell**».
//!
//! # Lo que esta caja de arena puede afirmar, y lo que no
//!
//! Puede afirmar que el instalador **respeta las rutas que se le dan**, que deja
//! el binario ejecutable, que no machaca una configuración existente y que dice
//! la frase del colector. Son las comprobaciones 2 y 5 de RPT-054 §8.
//!
//! **No puede afirmar nada sobre el ciclo de vida del servicio.** «Matar el
//! proceso y ver que vuelve» es una afirmación sobre `systemd`, y `systemd` no
//! corre en un directorio de `/tmp`. Eso es PA-117 y exige contenedor o máquina
//! virtual con `systemd` como PID 1 (RPT-062 §5).
//!
//! Juntarlas daría un verde que cubre dos comprobaciones y se lee como si
//! cubriera tres.

use std::path::Path;
use std::process::Command;

/// Lo que la caja de arena puede decir del instalador.
///
/// Tres estados y no dos, por lo de siempre: «no se pudo comprobar» no es «pasó»
/// (RPT-006 §4). Un arnés que no encuentra `sh` y devuelve verde es peor que uno
/// que no existe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resultado {
    /// Se comprobó todo y se cumple. Lleva las afirmaciones verificadas.
    Conforme(Vec<String>),
    /// Se comprobó y algo no se cumple.
    ViolacionDetectada(Vec<String>),
    /// No se pudo comprobar, y por qué.
    ComprobacionImposible(String),
}

/// Ejecuta el instalador contra un árbol de destino desechable.
///
/// # Errores
///
/// No devuelve `Err`: los fallos del entorno son
/// [`Resultado::ComprobacionImposible`], que es información y no ausencia de
/// ella.
#[must_use]
pub fn probar(raiz_repo: &Path) -> Resultado {
    if !cfg!(unix) {
        return Resultado::ComprobacionImposible(
            "el instalador es un guion de shell: esta comprobacion exige un sistema tipo Unix"
                .to_owned(),
        );
    }

    let artefacto = raiz_repo.join("target/paquete/eje-agente");
    if !artefacto.join("instalar.sh").is_file() {
        return Resultado::ComprobacionImposible(format!(
            "no hay artefacto en {}. Ejecuta antes: cargo xtask empaquetar",
            artefacto.display()
        ));
    }

    match Command::new("sh").arg("-c").arg("exit 0").status() {
        Ok(estado) if estado.success() => {}
        _ => {
            return Resultado::ComprobacionImposible(
                "no hay `sh` ejecutable en este sistema".to_owned(),
            );
        }
    }

    let arena = std::env::temp_dir().join("eje-latam-arena-instalador");
    let _ = std::fs::remove_dir_all(&arena);

    let mut fallos = Vec::new();
    let mut comprobado = Vec::new();

    // --- Primera instalación, sobre un destino virgen -----------------------
    let primera = match ejecutar(&artefacto, &arena) {
        Ok(salida) => salida,
        Err(motivo) => return Resultado::ComprobacionImposible(motivo),
    };

    for (ruta, que) in [
        (arena.join("bin/eje-agente"), "el binario"),
        (arena.join("unidad/eje-agente.service"), "la unidad"),
        (
            arena.join("conf/configuracion-sensor.toml.ejemplo"),
            "la plantilla de configuracion",
        ),
    ] {
        if ruta.is_file() {
            comprobado.push(format!("{que} aterriza en {}", ruta.display()));
        } else {
            fallos.push(format!("{que} no llego a {}", ruta.display()));
        }
    }

    if arena.join("datos").is_dir() {
        comprobado.push("el directorio de datos se crea".to_owned());
    } else {
        fallos.push("el directorio de datos no se creo".to_owned());
    }

    match ejecutable(&arena.join("bin/eje-agente")) {
        Some(true) => comprobado.push("el binario queda ejecutable".to_owned()),
        Some(false) => fallos.push("el binario NO quedo ejecutable".to_owned()),
        None => fallos.push("no se pudieron leer los permisos del binario".to_owned()),
    }

    // El grito, en una instalacion recien hecha. RPT-054 §4.1 lo ratifico para el
    // colector ausente; desde RPT-077 el asunto es mas grande y lo contiene: sin
    // configuracion firmada el sensor no vigila nada en absoluto.
    if primera.contains("TODAVIA NO VIGILA NADA") {
        comprobado.push("declara a gritos que aun no hay configuracion firmada".to_owned());
    } else {
        fallos.push(
            "una instalacion recien hecha NO avisa de que falta la configuracion \
             firmada: quedaria un sensor instalado que no mira nada y nadie lo sabria"
                .to_owned(),
        );
    }

    // Y NO se inventa una. RPT-077: la clave con la que se firma no vive en el
    // sensor, asi que un instalador que escribiera este fichero produciria uno que
    // no verifica — y el agente lo leeria como manipulacion, que es peor que la
    // ausencia que venia a resolver.
    if arena.join("conf/agente.conf.firmado").exists() {
        fallos.push(
            "el instalador FABRICO una configuracion firmada: no puede firmarla, \
             asi que el agente la leera como manipulacion"
                .to_owned(),
        );
    } else {
        comprobado.push("no fabrica la configuracion firmada que no puede firmar".to_owned());
    }

    // --- Segunda instalación, con la configuración firmada ya puesta --------
    //
    // Es la que de verdad importa reinstalando: pisarla dejaria al sensor sin la
    // unica configuracion que obedece, y volver a emitirla exige la clave del
    // cliente, que puede estar a un pais de distancia.
    let firmada = b"EJE-CFG1 esto no verifica, y da igual: lo que se mide es que sobreviva";
    if std::fs::write(arena.join("conf/agente.conf.firmado"), firmada).is_err() {
        return Resultado::ComprobacionImposible(
            "no se pudo escribir la configuracion de la segunda vuelta".to_owned(),
        );
    }

    if ejecutar(&artefacto, &arena).is_err() {
        return Resultado::ComprobacionImposible(
            "la segunda instalacion no se pudo ejecutar".to_owned(),
        );
    }

    match std::fs::read(arena.join("conf/agente.conf.firmado")) {
        Ok(contenido) if contenido == firmada => {
            comprobado
                .push("una segunda instalacion NO machaca la configuracion firmada".to_owned());
        }
        Ok(_) => fallos.push(
            "la segunda instalacion PISO la configuracion firmada del cliente: \
             reinstalar dejaria el sensor sin nada que obedecer"
                .to_owned(),
        ),
        Err(_) => {
            fallos.push("la configuracion firmada desaparecio en la segunda instalacion".to_owned())
        }
    }

    // --- Un paquete tocado no se instala (RPT-073, PA-126) ------------------
    //
    // Es la prueba que convierte la comprobacion de integridad en algo mas que
    // adorno. Sin ella, un `sha256sum -c` que siempre dijera «OK» pasaria en
    // verde y nadie se enteraria hasta el dia del despliegue.
    match un_paquete_tocado_se_rechaza(&artefacto, &arena) {
        Ok(true) => {
            comprobado
                .push("un paquete alterado NO se instala, y no deja nada a medias".to_owned());
        }
        Ok(false) => fallos.push(
            "un paquete ALTERADO se instalo igual: la comprobacion de integridad \
             no protege nada"
                .to_owned(),
        ),
        Err(motivo) => return Resultado::ComprobacionImposible(motivo),
    }

    // --- Aislamiento: el guion no escribe fuera de sus variables ------------
    match rutas_absolutas_del_guion(&artefacto.join("instalar.sh")) {
        Ok(sueltas) if sueltas.is_empty() => {
            comprobado.push("todo destino del guion sale de una variable DESTINO_*".to_owned());
        }
        Ok(sueltas) => fallos.push(format!(
            "el guion instala en rutas absolutas fuera de las variables: {}",
            sueltas.join(", ")
        )),
        Err(motivo) => return Resultado::ComprobacionImposible(motivo),
    }

    let _ = std::fs::remove_dir_all(&arena);

    if fallos.is_empty() {
        Resultado::Conforme(comprobado)
    } else {
        Resultado::ViolacionDetectada(fallos)
    }
}

/// Copia el artefacto, le cambia un byte y comprueba que el instalador se niega.
///
/// RPT-073, PA-126. Devuelve `true` si se nego **y** no dejo nada instalado: son
/// dos afirmaciones distintas y la segunda es la que importa. Un instalador que
/// detecta el dano despues de copiar el binario deja el sistema con un fichero
/// que no es el que se envio y un codigo de salida que dice que fallo, y eso es
/// peor que no comprobar.
///
/// # Errores
///
/// `Err` si la copia o la ejecucion no se pueden hacer: no se sabe, y eso no es
/// haber comprobado.
fn un_paquete_tocado_se_rechaza(artefacto: &Path, arena: &Path) -> Result<bool, String> {
    let copia = arena.join("paquete-tocado");
    std::fs::create_dir_all(&copia)
        .map_err(|error| format!("no se pudo crear la copia del paquete: {error}"))?;

    let entradas = std::fs::read_dir(artefacto)
        .map_err(|error| format!("no se pudo leer {}: {error}", artefacto.display()))?;

    for entrada in entradas {
        let entrada = entrada.map_err(|error| format!("entrada ilegible: {error}"))?;
        let ruta = entrada.path();
        if ruta.is_file() {
            std::fs::copy(&ruta, copia.join(entrada.file_name()))
                .map_err(|error| format!("no se pudo copiar {}: {error}", ruta.display()))?;
        }
    }

    // Un byte de mas al final del binario: el dano que produce una transferencia
    // truncada al reves, y el que produciria una sustitucion descuidada.
    let binario = copia.join("eje-agente");
    let mut bytes = std::fs::read(&binario)
        .map_err(|error| format!("no se pudo leer el binario copiado: {error}"))?;
    bytes.push(0);
    std::fs::write(&binario, &bytes)
        .map_err(|error| format!("no se pudo alterar el binario copiado: {error}"))?;

    let destino = arena.join("tocado");
    let salida = Command::new("sh")
        .arg("instalar.sh")
        .current_dir(&copia)
        .env("DESTINO_BIN", destino.join("bin"))
        .env("DESTINO_CONF", destino.join("conf"))
        .env("DESTINO_DATOS", destino.join("datos"))
        .env("DESTINO_UNIDAD", destino.join("unidad"))
        .output()
        .map_err(|error| format!("no se pudo ejecutar el instalador alterado: {error}"))?;

    Ok(!salida.status.success() && !destino.join("bin/eje-agente").is_file())
}

/// Corre `instalar.sh` con los destinos apuntando dentro de la arena.
fn ejecutar(artefacto: &Path, arena: &Path) -> Result<String, String> {
    let salida = Command::new("sh")
        .arg("instalar.sh")
        .current_dir(artefacto)
        .env("DESTINO_BIN", arena.join("bin"))
        .env("DESTINO_CONF", arena.join("conf"))
        .env("DESTINO_DATOS", arena.join("datos"))
        .env("DESTINO_UNIDAD", arena.join("unidad"))
        .output()
        .map_err(|error| format!("no se pudo ejecutar instalar.sh: {error}"))?;

    if !salida.status.success() {
        return Err(format!(
            "instalar.sh termino con error:\n{}",
            String::from_utf8_lossy(&salida.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&salida.stdout).into_owned())
}

/// Si el fichero tiene bit de ejecución. `None` si no se pudo leer.
fn ejecutable(ruta: &Path) -> Option<bool> {
    let metadatos = std::fs::metadata(ruta).ok()?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        Some(metadatos.permissions().mode() & 0o111 != 0)
    }

    #[cfg(not(unix))]
    {
        let _ = metadatos;
        None
    }
}

/// Destinos de `install` que no salen de una variable `DESTINO_*`.
///
/// # Por qué se lee el guion y no sólo se observa la arena
///
/// La arena demuestra que el instalador **usa** las variables. No demuestra que
/// no escriba **además** en otro sitio: una línea que copie a `/etc/algo` pasaría
/// desapercibida, porque la arena no mira ahí.
///
/// Mirar `/etc` tampoco serviría — un fichero legítimo del sistema es
/// indistinguible de uno recién puesto. Lo que sí se puede afirmar es que
/// ninguna línea de instalación nombra una ruta absoluta.
fn rutas_absolutas_del_guion(ruta: &Path) -> Result<Vec<String>, String> {
    let contenido = std::fs::read_to_string(ruta)
        .map_err(|error| format!("no se pudo leer {}: {error}", ruta.display()))?;

    let sueltas = contenido
        .lines()
        .map(str::trim)
        .filter(|linea| linea.starts_with("install "))
        .flat_map(|linea| linea.split_whitespace().skip(1))
        // Los destinos entrecomillados llevan `$DESTINO_...`; una ruta absoluta
        // desnuda empieza por barra.
        .filter(|palabra| palabra.starts_with('/') || palabra.starts_with("\"/"))
        .map(str::to_owned)
        .collect();

    Ok(sueltas)
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn el_guion_del_artefacto_no_nombra_rutas_absolutas() {
        // Se comprueba sobre el texto que produce `empaquetar`, sin necesidad de
        // artefacto en disco: la propiedad es del guion, no de la ejecucion.
        let arena = std::env::temp_dir().join("eje-latam-guion-suelto");
        let _ = std::fs::create_dir_all(&arena);
        let ruta = arena.join("instalar.sh");
        std::fs::write(&ruta, crate::empaquetar::instalador()).expect("escribir");

        assert!(
            rutas_absolutas_del_guion(&ruta)
                .expect("se puede leer")
                .is_empty()
        );

        let _ = std::fs::remove_dir_all(&arena);
    }

    #[test]
    fn una_ruta_absoluta_colada_se_detecta() {
        // La prueba de que la comprobacion anterior comprueba algo.
        let arena = std::env::temp_dir().join("eje-latam-guion-con-ruta");
        let _ = std::fs::create_dir_all(&arena);
        let ruta = arena.join("instalar.sh");
        std::fs::write(
            &ruta,
            "#!/bin/sh\ninstall -m 0755 eje-agente /usr/local/bin/x\n",
        )
        .expect("escribir");

        let sueltas = rutas_absolutas_del_guion(&ruta).expect("se puede leer");
        assert_eq!(sueltas, vec!["/usr/local/bin/x".to_owned()]);

        let _ = std::fs::remove_dir_all(&arena);
    }

    #[test]
    fn sin_artefacto_no_se_declara_conforme() {
        // «No se pudo comprobar» no es «paso». Un arnes que no encuentra el
        // artefacto y devuelve verde es peor que uno que no existe.
        let vacio = std::env::temp_dir().join("eje-latam-sin-artefacto");
        let _ = std::fs::remove_dir_all(&vacio);
        std::fs::create_dir_all(&vacio).expect("crear");

        assert!(matches!(
            probar(&vacio),
            Resultado::ComprobacionImposible(_)
        ));

        let _ = std::fs::remove_dir_all(&vacio);
    }

    #[test]
    fn un_guion_ilegible_no_se_absuelve() {
        let inexistente = std::env::temp_dir().join("eje-latam-guion-que-no-esta.sh");
        let _ = std::fs::remove_file(&inexistente);

        assert!(rutas_absolutas_del_guion(&inexistente).is_err());
    }
}
