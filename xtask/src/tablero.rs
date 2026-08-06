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

/// Extrae el identificador `PA-nn` de una fila, si lo lleva.
///
/// Tolera el tachado de Markdown —`~~PA-07~~`— porque el tablero lo usa para los
/// cerrados y quitarlo obligaria a reescribir filas que ya estan bien.
fn identificador_de(fila: &str) -> Option<String> {
    let posicion = fila.find("PA-")?;
    let resto = &fila[posicion + 3..];
    let digitos: String = resto.chars().take_while(char::is_ascii_digit).collect();

    if digitos.is_empty() {
        return None;
    }

    Some(format!("PA-{digitos}"))
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
