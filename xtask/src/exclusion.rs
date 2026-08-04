//! Determina que lineas de un fichero Rust pertenecen a un bloque `#[cfg(test)]`.
//!
//! # Por que esto no es un `grep`
//!
//! La version PowerShell del guardian resolvia la exclusion asi:
//!
//! ```text
//! if ($Linea -match "#\[cfg\(test\)\]") { break }
//! ```
//!
//! `break` abandona el fichero completo al primer `#[cfg(test)]`. Si un modulo de
//! pruebas aparece a mitad de fichero, todo lo posterior queda sin revisar y el
//! guardian informa conformidad. Un guardian con falsos negativos silenciosos es
//! peor que ninguno: produce confianza injustificada (RPT-003 §9.5).
//!
//! Este modulo cuenta llaves y **reanuda** el analisis al cerrar el bloque. Para
//! contarlas correctamente hay que ignorar las que aparecen dentro de cadenas,
//! cadenas crudas, literales de caracter y comentarios.

/// Estado del analizador lexico mientras recorre el fichero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Estado {
    /// Codigo normal.
    Codigo,
    /// Comentario de linea, hasta el salto.
    ComentarioLinea,
    /// Comentario de bloque. Los comentarios de bloque anidan en Rust.
    ComentarioBloque(u32),
    /// Cadena entre comillas dobles.
    Cadena,
    /// Cadena cruda `r#"..."#`, con el numero de almohadillas de apertura.
    CadenaCruda(usize),
    /// Literal de caracter.
    Caracter,
}

/// Marca, para cada linea del fuente, si pertenece a un bloque `#[cfg(test)]`.
///
/// El vector devuelto tiene una entrada por linea, en orden.
#[must_use]
pub fn lineas_de_prueba(fuente: &str) -> Vec<bool> {
    let caracteres: Vec<char> = fuente.chars().collect();
    let total_lineas = fuente.lines().count().max(1);
    let mut excluida = vec![false; total_lineas];

    let mut estado = Estado::Codigo;
    let mut linea: usize = 0;
    let mut profundidad: i32 = 0;
    let mut cfg_test_pendiente = false;
    let mut profundidad_exclusion: Option<i32> = None;
    let mut indice: usize = 0;

    while indice < caracteres.len() {
        let actual = caracteres[indice];
        let siguiente = caracteres.get(indice + 1).copied();

        if profundidad_exclusion.is_some() && linea < excluida.len() {
            excluida[linea] = true;
        }

        match estado {
            Estado::Codigo => {
                match (actual, siguiente) {
                    ('/', Some('/')) => {
                        estado = Estado::ComentarioLinea;
                        indice += 2;
                        continue;
                    }
                    ('/', Some('*')) => {
                        estado = Estado::ComentarioBloque(1);
                        indice += 2;
                        continue;
                    }
                    _ => {}
                }

                if actual == '"' {
                    estado = Estado::Cadena;
                } else if actual == 'r' && es_apertura_cruda(&caracteres, indice) {
                    let almohadillas = contar_almohadillas(&caracteres, indice + 1);
                    estado = Estado::CadenaCruda(almohadillas);
                    indice += 1 + almohadillas + 1;
                    continue;
                } else if actual == '\'' && es_literal_caracter(&caracteres, indice) {
                    estado = Estado::Caracter;
                } else if actual == '#' && coincide_cfg_test(&caracteres, indice) {
                    cfg_test_pendiente = true;
                } else if actual == '{' {
                    if cfg_test_pendiente && profundidad_exclusion.is_none() {
                        profundidad_exclusion = Some(profundidad);
                        cfg_test_pendiente = false;
                        if linea < excluida.len() {
                            excluida[linea] = true;
                        }
                    }
                    profundidad += 1;
                } else if actual == '}' {
                    profundidad -= 1;
                    if profundidad_exclusion == Some(profundidad) {
                        if linea < excluida.len() {
                            excluida[linea] = true;
                        }
                        profundidad_exclusion = None;
                    }
                } else if actual == ';' && cfg_test_pendiente {
                    // El atributo acompanaba a un `use` o un `const`, no a un
                    // bloque. No abre exclusion.
                    cfg_test_pendiente = false;
                }
            }

            Estado::ComentarioLinea => {
                if actual == '\n' {
                    estado = Estado::Codigo;
                }
            }

            Estado::ComentarioBloque(nivel) => match (actual, siguiente) {
                ('/', Some('*')) => {
                    estado = Estado::ComentarioBloque(nivel + 1);
                    indice += 2;
                    continue;
                }
                ('*', Some('/')) => {
                    estado = if nivel <= 1 {
                        Estado::Codigo
                    } else {
                        Estado::ComentarioBloque(nivel - 1)
                    };
                    indice += 2;
                    continue;
                }
                _ => {}
            },

            Estado::Cadena => {
                if actual == '\\' {
                    indice += 2;
                    if let Some(saltado) = caracteres.get(indice - 1) {
                        if *saltado == '\n' {
                            linea += 1;
                        }
                    }
                    continue;
                }
                if actual == '"' {
                    estado = Estado::Codigo;
                }
            }

            Estado::CadenaCruda(almohadillas) => {
                if actual == '"' && cierra_cadena_cruda(&caracteres, indice + 1, almohadillas) {
                    estado = Estado::Codigo;
                    indice += 1 + almohadillas;
                    continue;
                }
            }

            Estado::Caracter => {
                if actual == '\\' {
                    indice += 2;
                    continue;
                }
                if actual == '\'' {
                    estado = Estado::Codigo;
                }
            }
        }

        if actual == '\n' {
            linea += 1;
        }
        indice += 1;
    }

    excluida
}

/// Indica si en `indice` comienza una cadena cruda `r"` o `r#"`.
fn es_apertura_cruda(caracteres: &[char], indice: usize) -> bool {
    let mut cursor = indice + 1;
    while caracteres.get(cursor) == Some(&'#') {
        cursor += 1;
    }
    caracteres.get(cursor) == Some(&'"')
}

/// Cuenta las almohadillas de apertura de una cadena cruda.
fn contar_almohadillas(caracteres: &[char], desde: usize) -> usize {
    let mut cursor = desde;
    while caracteres.get(cursor) == Some(&'#') {
        cursor += 1;
    }
    cursor - desde
}

/// Indica si tras la comilla de cierre vienen las almohadillas esperadas.
fn cierra_cadena_cruda(caracteres: &[char], desde: usize, almohadillas: usize) -> bool {
    (0..almohadillas).all(|desplazamiento| caracteres.get(desde + desplazamiento) == Some(&'#'))
}

/// Distingue un literal de caracter de una etiqueta de tiempo de vida.
///
/// `'a` en `&'a str` no es un literal; `'{'` si lo es, y sus llaves no deben
/// contarse.
fn es_literal_caracter(caracteres: &[char], indice: usize) -> bool {
    match caracteres.get(indice + 1) {
        Some('\\') => true,
        Some(_) => caracteres.get(indice + 2) == Some(&'\''),
        None => false,
    }
}

/// Reconoce el atributo `#[cfg(test)]` admitiendo espacios interiores.
fn coincide_cfg_test(caracteres: &[char], indice: usize) -> bool {
    let esperado = ['#', '[', 'c', 'f', 'g', '(', 't', 'e', 's', 't', ')', ']'];
    let mut cursor = indice;
    for objetivo in esperado {
        while caracteres.get(cursor).is_some_and(|c| c.is_whitespace()) {
            cursor += 1;
        }
        if caracteres.get(cursor) != Some(&objetivo) {
            return false;
        }
        cursor += 1;
    }
    true
}
