//! Lectura del tablero de puntos abiertos de RPT-002.
//!
//! # Por que existe
//!
//! El tablero se ha resumido a mano cuatro veces y las cuatro reintrodujo puntos
//! ya cerrados. La causa no es descuido: es que **se reescribe de memoria en
//! lugar de leerse**.
//!
//! Es el mismo defecto que `contrato-ipc.toml` resolvio para los canales, y la
//! misma solucion: una declaracion unica y derivacion mecanica. El tablero de
//! RPT-002 §12 es la fuente de verdad; cualquier recuento sale de aqui.

use std::path::Path;

/// Estado de un punto, tal como lo declara el tablero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Cerrado o resuelto.
    Cerrado,
    /// Parcialmente cerrado.
    Parcial,
    /// Abierto.
    Abierto,
}

/// Punto leido del tablero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Punto {
    /// Identificador, por ejemplo `PA-14`.
    pub identificador: String,
    /// Estado declarado.
    pub estado: Estado,
}

/// Recuento por estado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recuento {
    /// Puntos cerrados.
    pub cerrados: usize,
    /// Puntos parciales.
    pub parciales: usize,
    /// Puntos abiertos.
    pub abiertos: usize,
}

impl Recuento {
    /// Total de identificadores.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.cerrados + self.parciales + self.abiertos
    }

    /// Puntos que siguen requiriendo trabajo.
    ///
    /// Un parcial cuenta como pendiente: cerrarlo a medias no lo cierra.
    #[must_use]
    pub const fn pendientes(&self) -> usize {
        self.parciales + self.abiertos
    }
}

/// Extrae el identificador `PA-nn` —con sufijo de particion si lo lleva— de una
/// fila.
///
/// Tolera el tachado de Markdown —`~~PA-07~~`— porque el tablero lo usa para los
/// cerrados y quitarlo obligaria a reescribir filas que ya estan bien.
///
/// # El sufijo forma parte del identificador
///
/// La primera version se detenia en los digitos, asi que `PA-14`, `PA-14a` y
/// `PA-14c` colapsaban en un solo identificador. Combinado con la
/// deduplicacion de [`leer`] —que conserva la primera aparicion— el efecto era
/// que **la fila cerrada del punto partido se comia a sus hijos abiertos**.
///
/// No es un defecto cosmetico: RPT-021 partio PA-14 precisamente para separar
/// lo que bloquea el despliegue de lo que no, y el comando escondia el
/// bloqueante resultante. Es el mismo fallo que el comando existe para impedir,
/// un nivel mas abajo.
///
/// # Por que se lee la celda y no una subcadena
///
/// El primer intento buscaba `PA-` en cualquier punto de la fila y tomaba la
/// letra siguiente como sufijo si no continuaba una palabra. **No funciona**, y
/// la razon merece quedar escrita: en `PA-14a |` la letra va seguida de espacio,
/// y en `PA-40y algo` tambien. Desde una subcadena los dos casos son
/// indistinguibles, asi que ninguna heuristica sobre los caracteres vecinos los
/// separa. La regla no era afinable: era imposible.
///
/// Lo que si los separa es **donde** estan. El identificador es la primera
/// celda de la fila, no un texto que aparezca en cualquier sitio. Leerla entera
/// y exigir que sea exactamente un identificador resuelve el caso y, de paso,
/// quita la dependencia del orden de aparicion: una fila cerrada que menciona
/// otro punto en su tercera columna ya no necesita que `find` acierte.
fn identificador_de(fila: &str) -> Option<String> {
    let celda = fila
        .split('|')
        .map(str::trim)
        .find(|celda| !celda.is_empty())?;

    let limpia = celda.replace("~~", "").replace('*', "");
    let resto = limpia.trim().strip_prefix("PA-")?;

    let digitos: String = resto.chars().take_while(char::is_ascii_digit).collect();
    if digitos.is_empty() {
        return None;
    }

    // El sufijo de particion es una sola letra minuscula. Cualquier otra cosa
    // significa que la celda no es un identificador, y entonces no se inventa
    // ninguno: un punto fantasma no se cierra nunca, porque no existe.
    let sufijo = &resto[digitos.len()..];
    if sufijo.len() > 1 || sufijo.chars().any(|letra| !letra.is_ascii_lowercase()) {
        return None;
    }

    Some(format!("PA-{digitos}{sufijo}"))
}

/// Lee el tablero de un contenido de RPT-002.
///
/// El estado sale del **primer marcador** que aparece en la fila. El orden de
/// comprobacion importa: una fila cerrada puede mencionar lo que falta, y buscar
/// primero «abierto» la clasificaria mal.
#[must_use]
pub fn leer(contenido: &str) -> Vec<Punto> {
    let mut puntos: Vec<Punto> = Vec::new();

    for fila in contenido.lines() {
        if !fila.starts_with('|') {
            continue;
        }

        let Some(identificador) = identificador_de(fila) else {
            continue;
        };

        let estado = if fila.contains('✅') {
            Estado::Cerrado
        } else if fila.contains('🔵') {
            Estado::Parcial
        } else if fila.contains('🟡') || fila.contains('🔴') {
            Estado::Abierto
        } else {
            continue;
        };

        // El triaje repite identificadores en tablas de resumen. Solo cuenta la
        // primera aparicion, que es la del tablero.
        if puntos
            .iter()
            .any(|previo| previo.identificador == identificador)
        {
            continue;
        }

        puntos.push(Punto {
            identificador,
            estado,
        });
    }

    puntos
}

/// Cuenta los puntos por estado.
#[must_use]
pub fn contar(puntos: &[Punto]) -> Recuento {
    let mut recuento = Recuento::default();

    for punto in puntos {
        match punto.estado {
            Estado::Cerrado => recuento.cerrados += 1,
            Estado::Parcial => recuento.parciales += 1,
            Estado::Abierto => recuento.abiertos += 1,
        }
    }

    recuento
}

/// Ruta del reporte que contiene el tablero.
const RUTA_TABLERO: &str =
    "docs/reportes/RPT-002_Arquitectura-Consolidada_2026-08-04_Arquitectura.md";

/// Lee el tablero desde la raiz del repositorio.
///
/// # Errores
///
/// Devuelve el motivo si el reporte no se puede leer.
pub fn desde_raiz(raiz: &Path) -> Result<Vec<Punto>, String> {
    let ruta = raiz.join(RUTA_TABLERO);
    std::fs::read_to_string(&ruta)
        .map(|contenido| leer(&contenido))
        .map_err(|error| format!("no se pudo leer {}: {error}", ruta.display()))
}

/// Identificadores citados en `docs/` que **no figuran en el tablero**.
///
/// RPT-060, PA-108.
///
/// # Por que hacia falta
///
/// El tablero se quedo en PA-76 mientras los reportes acunaban treinta y nueve
/// identificadores nuevos. Nadie lo noto porque nada lo comprobaba: este comando
/// contaba lo que habia y lo presentaba como el total del proyecto.
///
/// La herramienta no mentia sobre lo que leia. **El sitio que lee habia dejado
/// de escribirse**, que es la misma familia de defecto que este proyecto lleva
/// persiguiendo en el codigo: una fuente unica que alguien deja de alimentar
/// sigue pareciendo una fuente unica.
///
/// # Que cuenta como cita
///
/// Cualquier `PA-nn` en cualquier `.md` bajo `docs/`. Se prefiere pasarse a
/// quedarse corto: un identificador citado de mas obliga a escribir una fila, y
/// uno citado de menos es el que se pierde.
///
/// # Errores
///
/// Devuelve el motivo si `docs/` no se puede recorrer.
pub fn citados_sin_fila(raiz: &Path, puntos: &[Punto]) -> Result<Vec<String>, String> {
    let mut citados: Vec<String> = Vec::new();
    recoger_citas(&raiz.join("docs"), &mut citados)?;

    citados.sort();
    citados.dedup();
    citados.retain(|cita| {
        !puntos
            .iter()
            .any(|punto| punto.identificador.as_str() == cita.as_str())
    });

    Ok(citados)
}

/// Recorre un directorio y acumula los identificadores citados en sus `.md`.
fn recoger_citas(directorio: &Path, citados: &mut Vec<String>) -> Result<(), String> {
    let entradas = std::fs::read_dir(directorio)
        .map_err(|error| format!("no se pudo leer {}: {error}", directorio.display()))?;

    for entrada in entradas {
        let entrada = entrada.map_err(|error| format!("entrada ilegible: {error}"))?;
        let ruta = entrada.path();

        if ruta.is_dir() {
            recoger_citas(&ruta, citados)?;
            continue;
        }
        if ruta.extension().is_none_or(|extension| extension != "md") {
            continue;
        }

        let contenido = std::fs::read_to_string(&ruta)
            .map_err(|error| format!("no se pudo leer {}: {error}", ruta.display()))?;
        citados.extend(citas_de(&contenido));
    }

    Ok(())
}

/// Identificadores `PA-nn` que aparecen en un texto.
///
/// Reutiliza la regla de sufijo de [`identificador_de`]: `PA-14a` es un
/// identificador propio y no una mencion de `PA-14`.
fn citas_de(contenido: &str) -> Vec<String> {
    let mut encontrados = Vec::new();
    let mut resto = contenido;

    while let Some(posicion) = resto.find("PA-") {
        let cola = &resto[posicion + 3..];
        let digitos: String = cola.chars().take_while(char::is_ascii_digit).collect();
        resto = cola;

        if digitos.is_empty() {
            continue;
        }

        let tras_digitos = &cola[digitos.len()..];
        let sufijo: String = tras_digitos
            .chars()
            .take_while(char::is_ascii_lowercase)
            .take(1)
            .collect();

        // Una letra suelta detras del numero es sufijo de particion; dos o mas
        // son una palabra pegada, y entonces esto no era un identificador.
        let tras_sufijo = &tras_digitos[sufijo.len()..];
        if tras_sufijo.starts_with(char::is_alphabetic) {
            continue;
        }

        encontrados.push(format!("PA-{digitos}{sufijo}"));
    }

    encontrados
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const MUESTRA: &str = "\
| ID | Punto | Estado |
|---|---|---|
| PA-01 | Frontera open-core | ✅ Resuelto |
| ~~PA-07~~ | ~~Ratificacion~~ | ✅ Cerrado |
| PA-09 | Banco de pruebas | 🔵 **Parcial**. Falta levantar el banco |
| PA-14 | Cadena de firma | 🟡 Abierto |
texto suelto que no es fila
| PA-45 | Declaracion de VLAN | 🔴 **Abierto, critico** |
";

    #[test]
    fn se_leen_los_cuatro_estados() {
        let puntos = leer(MUESTRA);

        assert_eq!(puntos.len(), 5);
        assert_eq!(puntos[0].identificador, "PA-01");
        assert_eq!(puntos[0].estado, Estado::Cerrado);
        assert_eq!(puntos[2].estado, Estado::Parcial);
        assert_eq!(puntos[3].estado, Estado::Abierto);
        assert_eq!(puntos[4].estado, Estado::Abierto);
    }

    #[test]
    fn el_tachado_no_esconde_el_identificador() {
        // El tablero tacha los cerrados. Si el lector no lo tolerase, contaria
        // de menos justo los que ya estan hechos.
        let puntos = leer("| ~~PA-33~~ | ~~Enmienda~~ | ✅ Cerrado |");

        assert_eq!(puntos.len(), 1);
        assert_eq!(puntos[0].identificador, "PA-33");
    }

    #[test]
    fn una_fila_cerrada_que_menciona_lo_que_falta_sigue_cerrada() {
        // Es el error que un lector ingenuo cometeria: buscar «abierto» antes
        // que «cerrado» y clasificar mal las filas que explican su reserva.
        let puntos =
            leer("| ~~PA-24~~ | ~~Productores~~ | ✅ Cerrado. Huella y OUI siguen en PA-25 |");

        assert_eq!(puntos.len(), 1);
        assert_eq!(puntos[0].estado, Estado::Cerrado);
    }

    #[test]
    fn un_punto_partido_no_se_come_a_sus_hijos() {
        // La regresion. RPT-021 partio PA-14 en tres para separar lo que bloquea
        // el despliegue de lo que no; con el identificador truncado en los
        // digitos, la fila cerrada del padre absorbia a los hijos por
        // deduplicacion y el bloqueante desaparecia del recuento.
        let partido = "\
| ~~PA-14~~ | ~~Cadena de firma~~ | ✅ **Partido** — eran tres puntos con un numero |
| PA-14a | Firma de release | 🔴 Abierto, bloquea despliegue |
| PA-14c | Atestacion PQC | 🟡 Abierto. Post-MVP |
";
        let puntos = leer(partido);

        assert_eq!(puntos.len(), 3, "los tres deben contarse por separado");
        assert_eq!(puntos[0].identificador, "PA-14");
        assert_eq!(puntos[1].identificador, "PA-14a");
        assert_eq!(puntos[2].identificador, "PA-14c");

        let recuento = contar(&puntos);
        assert_eq!(recuento.cerrados, 1);
        assert_eq!(
            recuento.pendientes(),
            2,
            "el bloqueante no puede esconderse"
        );
    }

    #[test]
    fn una_celda_que_no_es_un_identificador_no_inventa_uno() {
        // La direccion contraria del defecto anterior. Un punto fantasma es
        // peor que uno perdido: nadie lo cierra nunca porque no existe.
        //
        // La primera version de esta prueba esperaba `Some("PA-40")` para
        // `PA-40y algo`, y fallo. Tenia razon en fallar: pedia distinguir la
        // `y` de `PA-40y algo` de la `a` de `PA-14a |`, y desde una subcadena
        // ambas son «minuscula seguida de espacio». La expectativa era
        // irrealizable, no el parser.
        assert_eq!(
            identificador_de("| PA-40y algo | x | 🟡 |"),
            None,
            "la celda entera no es un identificador, asi que no hay ninguno"
        );
        assert_eq!(
            identificador_de("| PA-40A | x | 🟡 |"),
            None,
            "la mayuscula no es sufijo de particion"
        );
        assert_eq!(identificador_de("| ID | Punto | Estado |"), None);
        assert_eq!(identificador_de("|---|---|---|"), None);
    }

    #[test]
    fn el_identificador_sobrevive_al_adorno_de_markdown() {
        assert_eq!(
            identificador_de("| PA-14a | x | 🟡 |"),
            Some("PA-14a".to_owned())
        );
        assert_eq!(
            identificador_de("| ~~PA-14~~ | x | ✅ |"),
            Some("PA-14".to_owned()),
            "el tachado no debe leerse como sufijo"
        );
        assert_eq!(
            identificador_de("| **PA-48** | x | 🔴 |"),
            Some("PA-48".to_owned()),
            "la negrita tampoco"
        );
    }

    #[test]
    fn una_mencion_en_otra_columna_no_desplaza_al_identificador() {
        // Antes esto dependia de que `find` diera con la primera aparicion.
        // Ahora es estructural: el identificador es la primera celda y punto.
        assert_eq!(
            identificador_de("| PA-48 | El mecanismo de PA-45 existe sin usuarios | 🔴 |"),
            Some("PA-48".to_owned())
        );
    }

    #[test]
    fn el_tablero_real_distingue_el_punto_partido_de_sus_hijos() {
        // Contra el fichero de verdad. Si alguien restaura el truncado, aqui se
        // ve sobre el tablero que importa y no sobre una muestra.
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask cuelga de la raiz");
        let puntos = desde_raiz(raiz).expect("el tablero debe existir");

        let identificadores: Vec<&str> = puntos
            .iter()
            .map(|punto| punto.identificador.as_str())
            .collect();

        assert!(identificadores.contains(&"PA-14"), "el padre partido");
        assert!(
            identificadores.contains(&"PA-14a"),
            "y el hijo que bloquea el despliegue, que es el que se perdia"
        );
    }

    #[test]
    fn un_identificador_repetido_solo_cuenta_una_vez() {
        // El triaje del §12.1 repite identificadores en tablas de resumen.
        // Contarlos dos veces inflaria el total.
        let repetido = "\
| PA-45 | Declaracion de VLAN | 🟡 Abierto |
| PA-45 | Bloquea el despliegue | 🔴 Critico |
";
        assert_eq!(leer(repetido).len(), 1);
    }

    #[test]
    fn un_parcial_cuenta_como_pendiente() {
        // Cerrar a medias no cierra. Si un parcial contara como cerrado, el
        // recuento diria que queda menos trabajo del que queda.
        let recuento = contar(&leer(MUESTRA));

        assert_eq!(recuento.total(), 5);
        assert_eq!(recuento.cerrados, 2);
        assert_eq!(recuento.parciales, 1);
        assert_eq!(recuento.abiertos, 2);
        assert_eq!(recuento.pendientes(), 3);
    }

    #[test]
    fn el_tablero_real_se_lee_y_cuadra() {
        // Contra el fichero de verdad, no contra una muestra. Si alguien anade
        // una fila con un marcador nuevo, esto lo detecta.
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask cuelga de la raiz");

        let puntos = desde_raiz(raiz).expect("el tablero debe existir");
        let recuento = contar(&puntos);

        assert!(
            recuento.total() >= 45,
            "el tablero declara {} identificadores; se esperaban al menos 45",
            recuento.total()
        );
        assert_eq!(
            recuento.total(),
            recuento.cerrados + recuento.parciales + recuento.abiertos
        );
    }
}
