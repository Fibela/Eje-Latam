//! Comprueba que toda prueba escrita en el arbol se ejecuta de verdad.
//!
//! RPT-039 §8, PA-73.
//!
//! # El fallo que esto cierra
//!
//! Al implementar PA-72, dos funciones `#[test]` quedaron **anidadas dentro de
//! otra funcion**. `cargo test` emitio `warning: cannot test inner items` y
//! siguio adelante, informando 25 pruebas en verde. Dos pruebas escritas, cero
//! ejecutadas, suite conforme.
//!
//! Solo `-D warnings` en clippy lo convirtio en error, y esa defensa es
//! condicional: existe porque alguien escribio la lente `unnameable_test_items`.
//! Nada garantiza que haya una lente para el siguiente error de la misma familia
//! —un `mod pruebas;` que se deja de declarar, un fichero que sale del arbol, un
//! `#[cfg(feature)]` que nadie activa—, y todos comparten sintoma: **la suite
//! sigue verde con una prueba menos**.
//!
//! # Por que se comparan dos fuentes y no una cifra
//!
//! La alternativa evidente es declarar cuantas pruebas debe haber por modulo. Ya
//! se descarto: una cifra escrita a mano hay que mantenerla, y quien la mantiene
//! se equivoca. Aqui se comparan dos cosas que **ya existen** y que deberian
//! coincidir sin que nadie las cuide:
//!
//! - los atributos `#[test]` presentes en el arbol de fuentes;
//! - las pruebas que `cargo test -- --list` declara registradas.
//!
//! # La comparacion es una desigualdad, no una igualdad
//!
//! Falla solo si **hay mas en el arbol que registradas**, que es exactamente la
//! condicion de prueba fantasma. Al reves no es un fallo: las pruebas de
//! documentacion aparecen en la lista y no llevan `#[test]` en ningun sitio, y
//! una igualdad estricta las convertiria en un error el dia que alguien escriba
//! el primer ejemplo ejecutable.
//!
//! # Y son tres cifras, no dos
//!
//! La primera ejecucion real de esta herramienta acuso una prueba fantasma que
//! no existia. `eje-captura` tiene dos pruebas mutuamente excluyentes por
//! plataforma —una `#[cfg(target_os = "linux")]` y otra `#[cfg(not(...))]`—, de
//! modo que el arbol declara dieciseis y cualquier plataforma registra quince.
//! La acusacion se habria repetido para siempre, en Linux y en Windows, sin que
//! nadie hubiera hecho nada mal.
//!
//! Contar mejor no arregla eso: resolver un `#[cfg]` en general —caracteristicas,
//! arquitectura, sistema— es rehacer un trozo de `rustc`. Lo que si se puede es
//! **decir la verdad**: una prueba condicionada es `ComprobacionImposible` de
//! RPT-006 §4, ni conforme ni violacion, y se cuenta aparte. La desigualdad
//! compara solo las incondicionales.
//!
//! El coste esta escrito para que nadie lo descubra tarde: **una prueba
//! condicionada que se vuelva fantasma no se detecta**. A cambio, la herramienta
//! no acusa en falso — y una que grita sin motivo se aprende a ignorar, que es
//! peor que no tenerla.

use std::path::{Path, PathBuf};

/// Estado del analizador lexico.
///
/// Es el mismo problema que [`crate::exclusion`] resuelve para las llaves: hay
/// que ignorar lo que aparece dentro de cadenas, cadenas crudas, literales de
/// caracter y comentarios. Un `grep` contaria el `#[test]` de un comentario de
/// documentacion, y este mismo fichero lleva varios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estado {
    Codigo,
    ComentarioLinea,
    ComentarioBloque(u32),
    Cadena,
    CadenaCruda(usize),
    Caracter,
}

/// Sustituye por espacios todo lo que no es codigo.
///
/// Conserva la longitud y los saltos de linea para que las posiciones sigan
/// siendo comparables con el fuente original.
#[must_use]
pub fn solo_codigo(fuente: &str) -> String {
    let caracteres: Vec<char> = fuente.chars().collect();
    let mut salida = String::with_capacity(fuente.len());
    let mut estado = Estado::Codigo;
    let mut indice = 0usize;

    /// Anade el caracter si estamos en codigo, y un espacio si no.
    fn emitir(salida: &mut String, actual: char, en_codigo: bool) {
        if actual == '\n' {
            salida.push('\n');
        } else if en_codigo {
            salida.push(actual);
        } else {
            salida.push(' ');
        }
    }

    while indice < caracteres.len() {
        let actual = caracteres[indice];
        let siguiente = caracteres.get(indice + 1).copied();

        match estado {
            Estado::Codigo => {
                if actual == '/' && siguiente == Some('/') {
                    estado = Estado::ComentarioLinea;
                    salida.push(' ');
                    salida.push(' ');
                    indice += 2;
                    continue;
                }
                if actual == '/' && siguiente == Some('*') {
                    estado = Estado::ComentarioBloque(1);
                    salida.push(' ');
                    salida.push(' ');
                    indice += 2;
                    continue;
                }
                if actual == 'r' && (siguiente == Some('"') || siguiente == Some('#')) {
                    // Posible cadena cruda: `r"`, `r#"`, `r##"`...
                    let mut almohadillas = 0usize;
                    let mut mirada = indice + 1;
                    while caracteres.get(mirada) == Some(&'#') {
                        almohadillas += 1;
                        mirada += 1;
                    }
                    if caracteres.get(mirada) == Some(&'"') {
                        estado = Estado::CadenaCruda(almohadillas);
                        for _ in indice..=mirada {
                            salida.push(' ');
                        }
                        indice = mirada + 1;
                        continue;
                    }
                }
                if actual == '"' {
                    estado = Estado::Cadena;
                    salida.push(' ');
                    indice += 1;
                    continue;
                }
                if actual == '\'' {
                    // Un tiempo de vida (`'a`) no es un literal de caracter. Se
                    // distingue porque no lleva comilla de cierre a distancia de
                    // uno o dos caracteres.
                    let cierra_en_uno = caracteres.get(indice + 2) == Some(&'\'');
                    let cierra_escapado =
                        siguiente == Some('\\') && caracteres.get(indice + 3) == Some(&'\'');
                    if cierra_en_uno || cierra_escapado {
                        estado = Estado::Caracter;
                        salida.push(' ');
                        indice += 1;
                        continue;
                    }
                }
                salida.push(actual);
            }

            Estado::ComentarioLinea => {
                if actual == '\n' {
                    estado = Estado::Codigo;
                }
                emitir(&mut salida, actual, false);
            }

            Estado::ComentarioBloque(nivel) => {
                if actual == '/' && siguiente == Some('*') {
                    estado = Estado::ComentarioBloque(nivel + 1);
                    salida.push(' ');
                    salida.push(' ');
                    indice += 2;
                    continue;
                }
                if actual == '*' && siguiente == Some('/') {
                    estado = if nivel <= 1 {
                        Estado::Codigo
                    } else {
                        Estado::ComentarioBloque(nivel - 1)
                    };
                    salida.push(' ');
                    salida.push(' ');
                    indice += 2;
                    continue;
                }
                emitir(&mut salida, actual, false);
            }

            Estado::Cadena => {
                if actual == '\\' {
                    salida.push(' ');
                    if let Some(saltado) = caracteres.get(indice + 1) {
                        emitir(&mut salida, *saltado, false);
                    }
                    indice += 2;
                    continue;
                }
                if actual == '"' {
                    estado = Estado::Codigo;
                }
                emitir(&mut salida, actual, false);
            }

            Estado::CadenaCruda(almohadillas) => {
                if actual == '"' {
                    let cierra =
                        (1..=almohadillas).all(|paso| caracteres.get(indice + paso) == Some(&'#'));
                    if cierra {
                        estado = Estado::Codigo;
                        for _ in 0..=almohadillas {
                            salida.push(' ');
                        }
                        indice += almohadillas + 1;
                        continue;
                    }
                }
                emitir(&mut salida, actual, false);
            }

            Estado::Caracter => {
                if actual == '\\' {
                    salida.push(' ');
                    if let Some(saltado) = caracteres.get(indice + 1) {
                        emitir(&mut salida, *saltado, false);
                    }
                    indice += 2;
                    continue;
                }
                if actual == '\'' {
                    estado = Estado::Codigo;
                }
                emitir(&mut salida, actual, false);
            }
        }

        indice += 1;
    }

    salida
}

/// Pruebas halladas en un fuente, separadas por si son comparables.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Recuento {
    /// Pruebas que **deben** estar registradas en cualquier plataforma.
    pub incondicionales: usize,
    /// Pruebas bajo `#[cfg(...)]`: puede que en esta plataforma no existan.
    pub condicionadas: usize,
}

impl Recuento {
    /// Suma de ambas.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.incondicionales + self.condicionadas
    }
}

/// Cuenta los atributos `#[test]` de una sola linea de codigo ya filtrado.
fn atributos_test_en(linea: &str) -> usize {
    let caracteres: Vec<char> = linea.chars().collect();
    let mut total = 0usize;
    let mut indice = 0usize;

    while indice + 1 < caracteres.len() {
        if caracteres[indice] != '#' || caracteres[indice + 1] != '[' {
            indice += 1;
            continue;
        }

        let mut mirada = indice + 2;
        while caracteres.get(mirada).is_some_and(|c| c.is_whitespace()) {
            mirada += 1;
        }

        let palabra: String = caracteres.iter().skip(mirada).take(4).collect();

        if palabra == "test" {
            let mut cierre = mirada + 4;
            while caracteres.get(cierre).is_some_and(|c| c.is_whitespace()) {
                cierre += 1;
            }
            if caracteres.get(cierre) == Some(&']') {
                total += 1;
            }
        }

        indice += 2;
    }

    total
}

/// Una linea que es un atributo, cualquiera.
fn es_linea_de_atributo(linea: &str) -> bool {
    linea.trim_start().starts_with("#[")
}

/// El atributo de la linea dada convive con un `#[cfg(...)]` en el mismo item.
///
/// Los atributos de un item son lineas contiguas, tanto por encima como por
/// debajo: `#[cfg(...)]` puede ir antes o despues de `#[test]` y en ambos casos
/// condiciona la misma funcion. El recorrido para al primer renglon que no es un
/// atributo, que es donde empieza otro item.
fn condicionada(lineas: &[&str], posicion: usize) -> bool {
    let cfg_en = |linea: &str| linea.contains("#[cfg(");

    if cfg_en(lineas[posicion]) {
        return true;
    }

    let mut arriba = posicion;
    while arriba > 0 && es_linea_de_atributo(lineas[arriba - 1]) {
        arriba -= 1;
        if cfg_en(lineas[arriba]) {
            return true;
        }
    }

    let mut abajo = posicion;
    while abajo + 1 < lineas.len() && es_linea_de_atributo(lineas[abajo + 1]) {
        abajo += 1;
        if cfg_en(lineas[abajo]) {
            return true;
        }
    }

    false
}

/// Cuenta los atributos `#[test]` de un fuente Rust.
///
/// No cuenta `#[cfg(test)]`, que empieza por `cfg(` y no por `test`.
#[must_use]
pub fn contar_pruebas(fuente: &str) -> Recuento {
    let codigo = solo_codigo(fuente);
    let lineas: Vec<&str> = codigo.lines().collect();
    let mut recuento = Recuento::default();

    for (posicion, linea) in lineas.iter().enumerate() {
        let cuantas = atributos_test_en(linea);
        if cuantas == 0 {
            continue;
        }
        if condicionada(&lineas, posicion) {
            recuento.condicionadas += cuantas;
        } else {
            recuento.incondicionales += cuantas;
        }
    }

    recuento
}

/// Ficheros `.rs` bajo una raiz, en profundidad.
fn fuentes(raiz: &Path, encontrados: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !raiz.exists() {
        return Ok(());
    }

    for entrada in std::fs::read_dir(raiz)? {
        let ruta = entrada?.path();
        if ruta.is_dir() {
            // `target` contiene fuentes generadas y dependencias ajenas: contar
            // sus pruebas mezclaria el arbol del proyecto con el de otros.
            if ruta.file_name().is_some_and(|nombre| nombre == "target") {
                continue;
            }
            fuentes(&ruta, encontrados)?;
        } else if ruta.extension().is_some_and(|extension| extension == "rs") {
            encontrados.push(ruta);
        }
    }

    Ok(())
}

/// Recuento estatico de todo el arbol del proyecto.
///
/// # Errores
///
/// Cualquier fallo de lectura del arbol de fuentes.
pub fn en_el_arbol(raiz: &Path) -> std::io::Result<Vec<(PathBuf, Recuento)>> {
    let mut ficheros = Vec::new();
    for dominio in ["crates", "xtask"] {
        fuentes(&raiz.join(dominio), &mut ficheros)?;
    }
    ficheros.sort();

    let mut recuento = Vec::new();
    for ruta in ficheros {
        let fuente = std::fs::read_to_string(&ruta)?;
        let cuantas = contar_pruebas(&fuente);
        if cuantas.total() > 0 {
            recuento.push((ruta, cuantas));
        }
    }

    Ok(recuento)
}

/// Cuenta las pruebas que `cargo test -- --list` declara registradas.
#[must_use]
pub fn registradas(salida: &str) -> usize {
    salida
        .lines()
        .filter(|linea| linea.trim_end().ends_with(": test"))
        .count()
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn un_atributo_de_prueba_se_cuenta_una_vez() {
        assert_eq!(contar_pruebas("#[test]\nfn a() {}\n").incondicionales, 1);
        assert_eq!(
            contar_pruebas("#[test]\nfn a() {}\n#[test]\nfn b() {}\n").incondicionales,
            2
        );
    }

    #[test]
    fn cfg_test_no_es_una_prueba() {
        // Es el falso positivo mas probable: aparece en todos los modulos de
        // prueba del workspace y contarlo inflaria el recuento estatico hasta
        // hacer fallar el arbol limpio.
        assert_eq!(contar_pruebas("#[cfg(test)]\nmod pruebas {}\n").total(), 0);
        assert_eq!(contar_pruebas("#![cfg(test)]\n").total(), 0);
    }

    #[test]
    fn un_atributo_en_un_comentario_no_cuenta() {
        // Este mismo fichero documenta `#[test]` en su encabezado. Un `grep`
        // contaria esas menciones y el recuento estatico superaria al registrado
        // en un arbol perfectamente sano: la herramienta acusaria en falso.
        assert_eq!(contar_pruebas("// #[test]\nfn a() {}\n").total(), 0);
        assert_eq!(contar_pruebas("/// #[test]\nfn a() {}\n").total(), 0);
        assert_eq!(contar_pruebas("/* #[test] */\n").total(), 0);
        assert_eq!(contar_pruebas("/* /* #[test] */ */\n").total(), 0);
    }

    #[test]
    fn un_atributo_dentro_de_una_cadena_no_cuenta() {
        // Dos almohadillas y no una: el contenido lleva `"#`, que con una sola
        // cerraria la cadena antes de tiempo. Escrito asi, esta linea ejercita
        // ademas el caso que mas importa del analizador — una comilla seguida de
        // menos almohadillas de las que abrieron **no** cierra la cadena cruda.
        assert_eq!(contar_pruebas(r##"let a = "#[test]";"##).total(), 0);
        assert_eq!(contar_pruebas("let a = r#\"#[test]\"#;").total(), 0);
        assert_eq!(contar_pruebas("let a = r\"#[test]\";").total(), 0);
    }

    #[test]
    fn un_tiempo_de_vida_no_descarrila_el_analisis() {
        // `'a` no abre un literal de caracter. Si se tomara por uno, todo lo
        // posterior quedaria «dentro de una comilla» y las pruebas de ese
        // fichero desaparecerian del recuento — que es el fallo silencioso que
        // esta herramienta existe para impedir, cometido por ella misma.
        let fuente = "struct S<'a> { r: &'a str }\n#[test]\nfn a() {}\n";
        assert_eq!(contar_pruebas(fuente).incondicionales, 1);
    }

    #[test]
    fn un_literal_de_caracter_se_cierra_bien() {
        assert_eq!(
            contar_pruebas("let c = '\\'';\n#[test]\nfn a() {}\n").incondicionales,
            1
        );
        assert_eq!(
            contar_pruebas("let c = '}';\n#[test]\nfn a() {}\n").incondicionales,
            1
        );
    }

    #[test]
    fn se_admite_el_atributo_con_espacios() {
        assert_eq!(contar_pruebas("#[ test ]\nfn a() {}\n").incondicionales, 1);
    }

    #[test]
    fn otros_atributos_no_se_confunden_con_la_prueba() {
        assert_eq!(
            contar_pruebas("#[should_panic]\n#[testigo]\n#[tests]\n").total(),
            0
        );
    }

    #[test]
    fn de_la_lista_solo_cuentan_las_pruebas() {
        let salida = "\
pruebas::una: test
pruebas::otra: test
algo::bench: benchmark

2 tests, 1 benchmark
";
        assert_eq!(registradas(salida), 2);
    }

    #[test]
    fn el_arbol_real_declara_al_menos_una_prueba_por_crate_con_pruebas() {
        // Prueba de humo sobre el arbol de verdad: si el recorrido se rompiera
        // —una extension mal comparada, una raiz equivocada— el recuento seria
        // cero y la comparacion pasaria siempre, en silencio.
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask cuelga de la raiz del workspace");

        let recuento = en_el_arbol(raiz).expect("el arbol se lee");
        let total: usize = recuento.iter().map(|(_, cuantas)| cuantas.total()).sum();

        assert!(total > 100, "recuento sospechosamente bajo: {total}");
        assert!(
            recuento
                .iter()
                .any(|(ruta, _)| ruta.to_string_lossy().contains("guardian-cc")),
            "guardian-cc tiene pruebas y debe aparecer"
        );
    }

    #[test]
    fn una_prueba_bajo_cfg_no_se_cuenta_como_exigible() {
        // El falso positivo de la primera ejecucion real. `eje-captura` declara
        // dieciseis pruebas y cualquier plataforma registra quince, porque dos
        // son mutuamente excluyentes. Contarlas como exigibles convertiria la
        // herramienta en una acusacion permanente, y a una herramienta que acusa
        // siempre se le deja de hacer caso.
        let antes = contar_pruebas("#[cfg(target_os = \"linux\")]\n#[test]\nfn a() {}\n");
        assert_eq!(antes.incondicionales, 0);
        assert_eq!(antes.condicionadas, 1);

        // El `#[cfg]` tambien condiciona si va DESPUES: son atributos del mismo
        // item y el orden entre ellos no significa nada.
        let despues = contar_pruebas("#[test]\n#[cfg(unix)]\nfn a() {}\n");
        assert_eq!(despues.condicionadas, 1);
    }

    #[test]
    fn el_cfg_de_otro_item_no_condiciona_la_prueba() {
        // El recorrido para al primer renglon que no es un atributo. Si no lo
        // hiciera, el `#[cfg(test)]` del modulo de arriba marcaria como
        // condicionadas TODAS las pruebas del workspace, y la comparacion no
        // detectaria ya nada mientras seguia diciendo que si.
        let fuente = "#[cfg(test)]\nmod pruebas {\n    #[test]\n    fn a() {}\n}\n";
        let recuento = contar_pruebas(fuente);

        assert_eq!(recuento.incondicionales, 1, "{recuento:?}");
        assert_eq!(recuento.condicionadas, 0);
    }

    #[test]
    fn el_arbol_real_tiene_dos_pruebas_condicionadas_y_estan_donde_deben() {
        // Ancla el caso concreto que destapo el fallo. Si alguien las unifica o
        // anade otra, esto lo dice en lugar de que la cifra baile sola.
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask cuelga de la raiz del workspace");

        let recuento = en_el_arbol(raiz).expect("el arbol se lee");
        let condicionadas: Vec<&(PathBuf, Recuento)> = recuento
            .iter()
            .filter(|(_, cuantas)| cuantas.condicionadas > 0)
            .collect();

        let total: usize = condicionadas.iter().map(|(_, c)| c.condicionadas).sum();

        assert_eq!(total, 2, "{condicionadas:?}");
        assert!(
            condicionadas
                .iter()
                .all(|(ruta, _)| ruta.to_string_lossy().contains("eje-captura")),
            "{condicionadas:?}"
        );
    }
}
