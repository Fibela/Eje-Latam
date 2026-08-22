//! Cotejo de sellos de evidencia. RPT-061, PA-115.
//!
//! # Qué es esto
//!
//! La otra mitad del testigo externo de RPT-038. El agente emite el extremo de
//! su registro —`(asiento, sello)`— cada vez que cambia; el colector guarda la
//! serie, y **la discrepancia se ve fuera del equipo comprometido**, que es el
//! unico sitio donde puede verse.
//!
//! Hasta hoy nadie la guardaba: el vigia descartaba los sellos. Emitirlos sin
//! cotejarlos es la misma media pieza que era el latido sin vigia (RPT-052 §6).
//!
//! # La serie se indexa por (maquina, interfaz)
//!
//! Este modulo se escribio primero **con el defecto intacto** —indexando solo
//! por `HOSTNAME`, que era lo unico que el sello decia— para poder observar el
//! fallo antes de arreglarlo. Se observo en ejecucion real:
//!
//! ```text
//! SELLO BASE  LapTap-AF: extremo anotado en el asiento 100.
//! !! RECORTE  LapTap-AF: el registro tenia 100 asientos y ahora declara 40.
//! ```
//!
//! Dos agentes intactos en un mismo servidor, cada uno con su registro, acusados
//! de recorte por tener longitudes distintas. Ahora el sello declara su interfaz
//! y la serie se lleva por identidad completa. RPT-061.

use std::collections::BTreeMap;

use crate::{Identidad, cabecera};

/// Un sello tal como llego por el cable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelloRecibido {
    /// Maquina que lo emite.
    pub maquina: String,
    /// Interfaz que vigila, si la declara. RPT-061, PA-115.
    ///
    /// Opcional por lo mismo que en el latido: un agente anterior a RPT-061 no
    /// la manda, y su serie es la de la maquina sola —distinta de cualquier par
    /// con interfaz y perfectamente estable—. Descartar su sello lo dejaria sin
    /// testigo, que es lo contrario de lo que RPT-038 busca.
    pub interfaz: Option<String>,
    /// Ultimo asiento del registro de evidencia.
    pub asiento: u64,
    /// Extremo de la cadena de resumenes, en hexadecimal.
    pub extremo: String,
}

impl SelloRecibido {
    /// Identidad del sensor que lo emitio.
    #[must_use]
    pub fn identidad(&self) -> Identidad {
        Identidad::nueva(&self.maquina, self.interfaz.as_deref())
    }
}

/// Que dice el cotejo del sello que acaba de llegar.
///
/// # Los dos ultimos son acusaciones, y no son la misma
///
/// Un retroceso dice que el registro **encogio**: habia mas asientos y ahora
/// hay menos. Un extremo distinto para el mismo asiento dice que el registro
/// tiene la misma longitud y **contenido distinto**. La primera apunta a un
/// recorte; la segunda, a una reescritura. Presentarlas juntas mandaria a
/// buscar la cosa equivocada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cotejo {
    /// Primer sello de esta maquina. Se establece la linea base.
    ///
    /// No se afirma nada: sin serie previa no hay discrepancia posible.
    LineaBase,
    /// El registro creció. Lo normal.
    Avanza {
        /// Asiento que se tenia anotado.
        desde: u64,
        /// Asiento que trae este sello.
        hasta: u64,
    },
    /// Mismo asiento y mismo extremo.
    ///
    /// **No es sospechoso**: es un sensor sin alertas nuevas, que es el estado
    /// normal de un segmento tranquilo. La leccion de RPT-057 §1, aplicada aqui
    /// antes de repetir el error.
    SinCambios,
    /// El asiento **retrocedio**: el registro tiene menos asientos que antes.
    ///
    /// Es la acusacion que RPT-038 existe para producir. El ancla local no la
    /// detecta porque quien recorta puede recalcularla; esto si, porque la serie
    /// vive fuera de la maquina.
    Retroceso {
        /// Asiento anotado.
        visto: u64,
        /// Asiento recibido.
        recibido: u64,
    },
    /// El mismo asiento con **otro extremo**: el registro se reescribio.
    ExtremoDistinto {
        /// Asiento en discordia.
        asiento: u64,
        /// Extremo anotado.
        visto: String,
        /// Extremo recibido.
        recibido: String,
    },
}

impl Cotejo {
    /// Si esto acusa a alguien de haber tocado el registro.
    #[must_use]
    pub const fn acusa(&self) -> bool {
        matches!(self, Self::Retroceso { .. } | Self::ExtremoDistinto { .. })
    }
}

/// Analiza una linea de syslog y devuelve el sello si lo es.
///
/// Devuelve `None` para cualquier otra cosa. Falla cerrado por el mismo motivo
/// que el latido: un sello mal leido puede fijar una linea base falsa, y sobre
/// una linea base falsa toda la serie posterior acusa o absuelve al azar.
#[must_use]
pub fn analizar(linea: &str) -> Option<SelloRecibido> {
    let (maquina, mensaje) = cabecera(linea, "sello-de-evidencia")?;

    let mut asiento = None;
    let mut extremo = None;
    let mut interfaz = None;

    for campo in &mensaje {
        let (clave, valor) = campo.split_once('=')?;
        match clave {
            "asiento" => asiento = valor.parse().ok(),
            "sello" => extremo = Some(valor.to_owned()),
            "interfaz" => interfaz = Some(valor.to_owned()),
            _ => {}
        }
    }

    Some(SelloRecibido {
        maquina,
        interfaz,
        asiento: asiento?,
        extremo: extremo?,
    })
}

/// Serie de extremos anotada por **identidad**, no por maquina.
#[derive(Debug, Clone, Default)]
pub struct Testigo {
    anotado: BTreeMap<Identidad, (u64, String)>,
}

impl Testigo {
    /// Testigo sin nada anotado.
    #[must_use]
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Incorpora un sello y dice que se puede afirmar de el.
    pub fn cotejar(&mut self, sello: &SelloRecibido) -> Cotejo {
        let identidad = sello.identidad();
        let anterior = self.anotado.get(&identidad).cloned();

        self.anotado
            .insert(identidad, (sello.asiento, sello.extremo.clone()));

        let Some((visto, extremo_visto)) = anterior else {
            return Cotejo::LineaBase;
        };

        if sello.asiento < visto {
            return Cotejo::Retroceso {
                visto,
                recibido: sello.asiento,
            };
        }

        if sello.asiento == visto {
            if sello.extremo == extremo_visto {
                return Cotejo::SinCambios;
            }
            return Cotejo::ExtremoDistinto {
                asiento: visto,
                visto: extremo_visto,
                recibido: sello.extremo.clone(),
            };
        }

        Cotejo::Avanza {
            desde: visto,
            hasta: sello.asiento,
        }
    }

    /// Extremo anotado para una identidad, si hay alguno.
    #[must_use]
    pub fn anotado_de(&self, identidad: &Identidad) -> Option<(u64, &str)> {
        self.anotado
            .get(identidad)
            .map(|(asiento, extremo)| (*asiento, extremo.as_str()))
    }
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn linea(maquina: &str, interfaz: &str, asiento: u64, extremo: &str) -> String {
        format!(
            "<110>1 2026-08-13T10:00:00.000Z {maquina} eje-agente - sello-de-evidencia - \
             sello={extremo} asiento={asiento} interfaz={interfaz}"
        )
    }

    /// Sello de la interfaz `eth0`, que es la de casi todas estas pruebas.
    fn sello(maquina: &str, asiento: u64, extremo: &str) -> SelloRecibido {
        analizar(&linea(maquina, "eth0", asiento, extremo)).expect("linea valida")
    }

    /// La identidad que produce `sello`, para comparar sin repetir la notacion.
    fn quien(maquina: &str) -> Identidad {
        Identidad::nueva(maquina, Some("eth0"))
    }

    #[test]
    fn un_sello_se_lee_entero() {
        let leido = sello("sensor-uci", 42, "abc123");

        assert_eq!(leido.maquina, "sensor-uci");
        assert_eq!(leido.asiento, 42);
        assert_eq!(leido.extremo, "abc123");
        assert_eq!(leido.identidad(), quien("sensor-uci"));
    }

    #[test]
    fn un_latido_no_se_confunde_con_un_sello() {
        // Los dos llevan `sello=` y `asiento=`. Discriminarlos por esos campos es
        // el error que ya se cometio en las pruebas del ciclo; aqui se hace por
        // el identificador de mensaje.
        let latido = "<110>1 2026-08-13T10:00:00.000Z s eje-agente - latido-de-sensor - \
                      latido=1 interfaz=eth0 sello=abc asiento=1 intervaloMs=60000 \
                      condiciones=ninguna";

        assert_eq!(analizar(latido), None);
        assert!(
            crate::analizar(latido).is_some(),
            "y el latido sigue siendo latido"
        );
    }

    #[test]
    fn el_primer_sello_no_afirma_nada() {
        let mut testigo = Testigo::nuevo();
        assert_eq!(testigo.cotejar(&sello("s", 10, "aaa")), Cotejo::LineaBase);
        assert_eq!(testigo.anotado_de(&quien("s")), Some((10, "aaa")));
    }

    #[test]
    fn un_registro_que_crece_no_acusa_a_nadie() {
        let mut testigo = Testigo::nuevo();
        testigo.cotejar(&sello("s", 10, "aaa"));

        let cotejo = testigo.cotejar(&sello("s", 12, "bbb"));
        assert_eq!(
            cotejo,
            Cotejo::Avanza {
                desde: 10,
                hasta: 12
            }
        );
        assert!(!cotejo.acusa());
    }

    #[test]
    fn un_sensor_en_calma_repite_su_extremo_y_eso_no_es_sospechoso() {
        // La leccion de RPT-057 §1, aplicada antes de repetir el error: sin
        // alertas nuevas el registro no crece y el extremo es el mismo. Tomarlo
        // por manipulacion acusaria a todo sensor tranquilo.
        let mut testigo = Testigo::nuevo();
        testigo.cotejar(&sello("s", 10, "aaa"));

        let cotejo = testigo.cotejar(&sello("s", 10, "aaa"));
        assert_eq!(cotejo, Cotejo::SinCambios);
        assert!(!cotejo.acusa());
    }

    #[test]
    fn un_registro_que_encoge_acusa_de_recorte() {
        // El ataque que el ancla local no ve: parar el agente, recortar el
        // registro, recalcular el ancla, arrancar. El cotejo local dice
        // `Conforme` porque el atacante lo hizo bien; lo unico que lo delata es
        // que el colector tenia anotado un asiento mas alto (RPT-038).
        let mut testigo = Testigo::nuevo();
        testigo.cotejar(&sello("s", 100, "aaa"));

        let cotejo = testigo.cotejar(&sello("s", 40, "bbb"));
        assert_eq!(
            cotejo,
            Cotejo::Retroceso {
                visto: 100,
                recibido: 40
            }
        );
        assert!(cotejo.acusa());
    }

    #[test]
    fn el_mismo_asiento_con_otro_extremo_acusa_de_reescritura() {
        // Longitud igual, contenido distinto. Es otra acusacion y manda a buscar
        // otra cosa: no falta nada, cambio algo.
        let mut testigo = Testigo::nuevo();
        testigo.cotejar(&sello("s", 10, "aaa"));

        let cotejo = testigo.cotejar(&sello("s", 10, "bbb"));
        assert_eq!(
            cotejo,
            Cotejo::ExtremoDistinto {
                asiento: 10,
                visto: "aaa".to_owned(),
                recibido: "bbb".to_owned()
            }
        );
        assert!(cotejo.acusa());
    }

    #[test]
    fn dos_maquinas_distintas_no_se_pisan() {
        let mut testigo = Testigo::nuevo();
        testigo.cotejar(&sello("hospital-a", 100, "aaa"));

        assert_eq!(
            testigo.cotejar(&sello("hospital-b", 3, "bbb")),
            Cotejo::LineaBase,
            "cada maquina lleva su propia serie"
        );
    }

    // -----------------------------------------------------------------------
    // La identidad compuesta en el sello — RPT-061, PA-115
    // -----------------------------------------------------------------------

    #[test]
    fn dos_agentes_en_una_maquina_se_acusan_entre_ellos() {
        // ESTA PRUEBA DESCRIBE UN DEFECTO, NO UNA GARANTIA.
        //
        // El sello no lleva interfaz, asi que dos agentes del mismo servidor
        // —un sensor por segmento— comparten serie. Sus registros son
        // independientes y de longitudes distintas, de modo que sus sellos se
        // leen como un registro que encoge y crece solo.
        //
        // Nadie ha tocado nada. La acusacion es falsa, y va dirigida a una
        // maquina que esta funcionando bien.
        //
        // Cuando PA-115 se corrija, esta prueba tiene que **dejar de pasar** y
        // se sustituira por su contraria. Esta escrita para eso.
        let mut testigo = Testigo::nuevo();

        // El agente de `eth0` lleva 100 asientos; el de `eth1`, 40.
        testigo.cotejar(&sello("perimetro", 100, "extremo-de-eth0"));
        let cotejo = testigo.cotejar(&sello("perimetro", 40, "extremo-de-eth1"));

        assert!(
            cotejo.acusa(),
            "hoy el cotejo acusa de manipulacion a dos sensores sanos: {cotejo:?}"
        );
        assert_eq!(
            cotejo,
            Cotejo::Retroceso {
                visto: 100,
                recibido: 40
            }
        );
    }

    #[test]
    fn y_con_registros_del_mismo_tamano_la_acusacion_es_todavia_peor() {
        // Dos agentes recien instalados anexan su primera alerta casi a la vez.
        // Mismo asiento, extremos distintos: la acusacion mas grave que este
        // sistema sabe emitir —«alguien reescribio el registro»— sobre dos
        // sensores que no han hecho nada malo.
        let mut testigo = Testigo::nuevo();

        testigo.cotejar(&sello("perimetro", 1, "extremo-de-eth0"));
        let cotejo = testigo.cotejar(&sello("perimetro", 1, "extremo-de-eth1"));

        assert!(
            matches!(cotejo, Cotejo::ExtremoDistinto { .. }),
            "{cotejo:?}"
        );
    }
}
