//! Detector de ausencia de latidos. RPT-057, PA-105.
//!
//! # Que es esto y que no
//!
//! Es el **detector de referencia**: la mitad de PA-104 que no vive en el sensor.
//! El agente late (RPT-052, RPT-053) y hasta ahora nadie vigilaba ese latido, con
//! lo que emitirlo no cerraba nada — RPT-052 §6 lo dejo dicho antes de escribir
//! una linea: emitir sin vigilar es peor que no emitir, porque da el punto por
//! resuelto y deja a la sala igual de ciega.
//!
//! **No es el SIEM del cliente.** Es la implementacion mas pequena que permite
//! apagar un sensor y comprobar aqui que alguien se entera, que es la condicion
//! literal de cierre de PA-104. Tambien sirve de especificacion ejecutable para
//! quien lo implemente en su propia herramienta: una regla escrita en prosa se
//! interpreta; esta se ejecuta.
//!
//! # El reloj es el del colector, no el de la linea
//!
//! La ausencia se calcula con **la hora de llegada**, no con la marca de tiempo
//! que trae el mensaje. Dos motivos, y los dos son de este proyecto:
//!
//! - La marca la escribe el sensor, y ya se vio un reloj de pared retrocediendo
//!   (RPT-053 §3). Un sensor con la hora mal desplazaria su propia ventana.
//! - Syslog no esta autenticado. Quien pueda escribir en el canal puede fechar
//!   como quiera, y fechar en el futuro compraria silencio.
//!
//! # Lo que este detector NO puede ver
//!
//! Un sensor que se instalo y **nunca hablo** no existe para el. No hay ausencia
//! que detectar donde no hubo presencia: es «no se sabe», no «no hay»
//! (RPT-006 §4).
//!
//! Por eso el censo ([`Vigia::esperar`]) es la unica forma de cubrir ese caso, y
//! por eso el censo tiene que salir de la lista de sensores desplegados y no de
//! lo que el colector haya oido. Sin censo, este detector cubre «se apago» y no
//! cubre «nunca arranco».

pub mod sellos;

use std::collections::{BTreeMap, BTreeSet};

/// Intervalos que se dejan pasar antes de declarar ausente a un sensor.
///
/// **Hipotesis declarada, no medida** — la misma deuda que PA-41. Uno solo
/// convertiria cualquier reordenacion o congestion del colector en una llamada;
/// muchos alargan el tiempo en que un hospital no esta vigilado sin saberlo.
///
/// Tres es lo habitual en supervision y no lo justifica ninguna medida nuestra.
pub const TOLERANCIA_INTERVALOS: i64 = 3;

/// Identidad de un sensor en la sala: la maquina **y** la interfaz que vigila.
///
/// RPT-059, PA-113.
///
/// # Por que el par y no la maquina
///
/// Un servidor perimetral con un agente por segmento es un despliegue normal en
/// una planta grande. Todos esos agentes comparten `HOSTNAME`, y con la maquina
/// como clave unica **el latido de uno taparia la muerte del otro** — el mismo
/// fallo que PA-104 existe para impedir, y el mismo error de RPT-058 §2 visto
/// desde el otro lado: alli se tomo la parte por el todo, aqui el todo por la
/// parte.
///
/// # La interfaz es opcional, y eso no es un descuido
///
/// Un agente anterior a RPT-059 no la declara. Su identidad es la maquina sola,
/// que es distinta de cualquier par con interfaz y perfectamente estable: se le
/// vigila igual. Rellenarla con una cadena vacia lo haria indistinguible de un
/// agente que declara una interfaz sin nombre, y eso es la clase de mentira
/// pequena que este proyecto persigue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Identidad {
    /// Maquina, del campo `HOSTNAME` de RFC 5424.
    pub maquina: String,
    /// Interfaz que vigila, si el latido la declara.
    pub interfaz: Option<String>,
}

impl Identidad {
    /// Identidad a partir de sus dos partes.
    #[must_use]
    pub fn nueva(maquina: &str, interfaz: Option<&str>) -> Self {
        Self {
            maquina: maquina.to_owned(),
            interfaz: interfaz.map(str::to_owned),
        }
    }

    /// Lee `maquina/interfaz`, o `maquina` a secas.
    ///
    /// Es la forma en que el censo nombra a un sensor, y la misma que se imprime.
    /// Que leer y escribir usen la misma notacion evita que alguien declare en el
    /// censo algo que el vigia nunca podra emparejar — que es exactamente lo que
    /// paso en la primera prueba de fuego de RPT-058.
    #[must_use]
    pub fn desde_texto(texto: &str) -> Self {
        match texto.split_once('/') {
            Some((maquina, interfaz)) => Self::nueva(maquina, Some(interfaz)),
            None => Self::nueva(texto, None),
        }
    }
}

impl std::fmt::Display for Identidad {
    fn fmt(&self, salida: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.interfaz {
            Some(interfaz) => write!(salida, "{}/{interfaz}", self.maquina),
            None => write!(salida, "{}", self.maquina),
        }
    }
}

/// Un latido tal como llego por el cable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatidoRecibido {
    /// Sensor que lo emite, del campo `HOSTNAME` de RFC 5424.
    pub maquina: String,
    /// Interfaz que vigila, si la declara. RPT-059, PA-113.
    pub interfaz: Option<String>,
    /// Contador monotono de latidos de ese sensor.
    pub contador: u64,
    /// Ultimo asiento del registro de evidencia.
    pub asiento: u64,
    /// Extremo de la cadena de resumenes.
    pub sello: String,
    /// Cada cuanto dice el sensor que va a latir.
    pub intervalo_ms: i64,
    /// Condiciones emisibles activas, por nombre. Vacio significa ninguna.
    pub condiciones: Vec<String>,
}

impl LatidoRecibido {
    /// Identidad del sensor que lo emitio.
    #[must_use]
    pub fn identidad(&self) -> Identidad {
        Identidad::nueva(&self.maquina, self.interfaz.as_deref())
    }
}

/// Que se puede decir del latido que acaba de llegar.
///
/// # Por que no es un booleano
///
/// «Es nuevo» y «no lo es» esconden tres situaciones que exigen respuestas
/// distintas, y una de ellas no se puede resolver mirando el mensaje.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acogida {
    /// Primer latido de este sensor. Se establece la linea base.
    ///
    /// **No se afirma nada todavia.** Un sensor recien visto no puede estar
    /// ausente ni ser sospechoso: no hay contra que comparar.
    LineaBase,
    /// El contador avanzo exactamente uno. Lo normal.
    Continua,
    /// El contador avanzo mas de uno: se perdieron latidos por el camino.
    ///
    /// El sensor esta vivo —este mensaje lo prueba— pero el canal se comio
    /// lineas. Importa porque la serie es contigua por construccion: el agente
    /// solo consume el numero tras un envio correcto (RPT-057 §3).
    HuecoEnLaSerie {
        /// Cuantos latidos no llegaron.
        perdidos: u64,
    },
    /// El contador no avanzo, o retrocedio.
    ///
    /// # Y aqui el detector NO decide
    ///
    /// Son dos cosas con la misma forma: el agente reinicio —el contador vuelve
    /// a empezar, porque no sobrevive al proceso— o alguien esta reproduciendo
    /// un latido capturado.
    ///
    /// Elegir una seria inventarse la respuesta. Un reinicio presentado como
    /// ataque manda a alguien a responder a un incidente que no existe; una
    /// repeticion presentada como reinicio deja a la sala en verde mientras el
    /// sensor esta silenciado. Se declaran las dos y decide un humano.
    ReinicioORepeticion {
        /// Contador que se habia visto antes.
        visto: u64,
        /// Contador que trae este mensaje.
        recibido: u64,
    },
}

/// Lo que el vigia puede decir de un sensor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vigilancia {
    /// Latio dentro de su ventana.
    Vivo {
        /// Sensor.
        identidad: Identidad,
        /// Milisegundos desde su ultimo latido.
        hace_ms: i64,
    },
    /// Paso su ventana sin latir. **Esto es la alarma.**
    Ausente {
        /// Sensor.
        identidad: Identidad,
        /// Milisegundos desde su ultimo latido.
        hace_ms: i64,
        /// Cuanto se le permitia callar.
        ventana_ms: i64,
    },
    /// Esta en el censo y **nunca ha dicho nada**.
    ///
    /// Distinto de `Ausente`: aquel se apago, este quiza nunca arranco, o quiza
    /// nunca se instalo. La diferencia importa porque manda a sitios distintos:
    /// a mirar un sensor caido, o a mirar una instalacion que no se termino.
    NuncaVisto {
        /// Sensor.
        identidad: Identidad,
    },
}

/// Lo que se recuerda de cada sensor.
#[derive(Debug, Clone)]
struct Sensor {
    contador: u64,
    ultimo_ms: i64,
    intervalo_ms: i64,
    condiciones: Vec<String>,
}

/// Reconoce la cabecera de RFC 5424 y devuelve `(maquina, campos del mensaje)`.
///
/// Compartido por el latido y el sello. Escribir la comprobacion dos veces seria
/// tener el mismo hecho en dos sitios, que es el defecto que este proyecto lleva
/// el dia entero arreglando en otras superficies.
///
/// # Falla cerrado
///
/// Un `HOSTNAME` con espacios desplaza los campos y hace que las comprobaciones
/// de posicion fallen. Eso es correcto: preferimos no reconocer la linea a
/// atribuirsela a la maquina equivocada.
pub(crate) fn cabecera<'a>(linea: &'a str, identificador: &str) -> Option<(String, Vec<&'a str>)> {
    let campos: Vec<&str> = linea.split(' ').collect();

    // <pri>1 TIMESTAMP HOSTNAME APP-NAME PROCID MSGID STRUCTURED-DATA MSG...
    if campos.len() < 8 || campos[3] != "eje-agente" || campos[5] != identificador {
        return None;
    }

    let maquina = (*campos.get(2)?).to_owned();
    if maquina.is_empty() || maquina == "-" {
        return None;
    }

    Some((maquina, campos[7..].to_vec()))
}

/// Analiza una linea de syslog y devuelve el latido si lo es.
///
/// Devuelve `None` para cualquier otra cosa —alertas, sellos, transiciones, o
/// texto que no compusimos nosotros—, y tambien para un latido malformado.
///
/// # Falla cerrado
///
/// No se intenta rescatar una linea a medias. Un latido mal leido es peor que
/// uno ignorado: el ignorado acaba disparando la ausencia, que es la respuesta
/// segura; el mal leido puede fijar una linea base falsa y **comprar silencio**.
///
/// Un `HOSTNAME` con espacios desplaza los campos y hace que las comprobaciones
/// de posicion fallen. Eso es correcto: preferimos no reconocer ese latido a
/// atribuirselo a la maquina equivocada.
#[must_use]
pub fn analizar(linea: &str) -> Option<LatidoRecibido> {
    let (maquina, mensaje) = cabecera(linea, "latido-de-sensor")?;

    let mut contador = None;
    let mut asiento = None;
    let mut sello = None;
    let mut intervalo_ms = None;
    let mut condiciones = None;
    let mut interfaz = None;

    for campo in &mensaje {
        let (clave, valor) = campo.split_once('=')?;
        match clave {
            "latido" => contador = valor.parse().ok(),
            "asiento" => asiento = valor.parse().ok(),
            "sello" => sello = Some(valor.to_owned()),
            "intervaloMs" => intervalo_ms = valor.parse().ok(),
            // Opcional: un agente anterior a RPT-059 no la manda, y su ausencia
            // es una identidad valida (la maquina sola), no un latido roto.
            "interfaz" => interfaz = Some(valor.to_owned()),
            // «ninguna» es una afirmacion —se miraron las ocho y no habia
            // ninguna activa—, y por eso se traduce a lista vacia y no a
            // ausencia de dato.
            "condiciones" => {
                condiciones = Some(if valor == "ninguna" {
                    Vec::new()
                } else {
                    valor.split(',').map(str::to_owned).collect()
                });
            }
            // Un campo que no conocemos no invalida el latido: el agente puede
            // ganar campos y este detector seguir sirviendo. Lo que no se
            // tolera es que falte uno de los que se usan.
            _ => {}
        }
    }

    let intervalo_ms = intervalo_ms?;
    if intervalo_ms <= 0 {
        // Un intervalo de cero o negativo haria la ventana inutil y la ausencia
        // permanente o imposible. No se corrige: se descarta.
        return None;
    }

    Some(LatidoRecibido {
        maquina,
        interfaz,
        contador: contador?,
        asiento: asiento?,
        sello: sello?,
        intervalo_ms,
        condiciones: condiciones?,
    })
}

/// Estado del vigia: que sensores se esperan y que se sabe de cada uno.
#[derive(Debug, Clone, Default)]
pub struct Vigia {
    censo: BTreeSet<Identidad>,
    sensores: BTreeMap<Identidad, Sensor>,
}

impl Vigia {
    /// Vigia sin censo: solo sabra de los sensores que hablen.
    #[must_use]
    pub fn nuevo() -> Self {
        Self::default()
    }

    /// Declara un sensor que **debe** estar informando.
    ///
    /// Es lo unico que permite detectar el que nunca arranco. Sale de la lista de
    /// despliegue del cliente, no de lo que el colector haya oido: un censo
    /// deducido de quien habla no puede echar de menos a quien nunca hablo.
    /// Se nombra como `maquina/interfaz`, o `maquina` a secas.
    pub fn esperar(&mut self, sensor: &str) {
        self.censo.insert(Identidad::desde_texto(sensor));
    }

    /// Incorpora un latido y dice que se puede afirmar de el.
    ///
    /// `ahora_ms` es la hora **del colector** al recibirlo. Ver el encabezado del
    /// modulo.
    pub fn observar(&mut self, latido: &LatidoRecibido, ahora_ms: i64) -> Acogida {
        let identidad = latido.identidad();
        let anterior = self.sensores.get(&identidad).map(|s| s.contador);

        self.sensores.insert(
            identidad,
            Sensor {
                contador: latido.contador,
                ultimo_ms: ahora_ms,
                intervalo_ms: latido.intervalo_ms,
                condiciones: latido.condiciones.clone(),
            },
        );

        let Some(visto) = anterior else {
            return Acogida::LineaBase;
        };

        if latido.contador <= visto {
            return Acogida::ReinicioORepeticion {
                visto,
                recibido: latido.contador,
            };
        }

        match latido.contador - visto {
            1 => Acogida::Continua,
            saltados => Acogida::HuecoEnLaSerie {
                perdidos: saltados - 1,
            },
        }
    }

    /// Estado de todos los sensores conocidos y esperados.
    #[must_use]
    pub fn revisar(&self, ahora_ms: i64) -> Vec<Vigilancia> {
        let mut salida: Vec<Vigilancia> = self
            .sensores
            .iter()
            .map(|(identidad, sensor)| {
                let hace_ms = ahora_ms.saturating_sub(sensor.ultimo_ms);
                let ventana_ms = sensor.intervalo_ms.saturating_mul(TOLERANCIA_INTERVALOS);

                if hace_ms > ventana_ms {
                    Vigilancia::Ausente {
                        identidad: identidad.clone(),
                        hace_ms,
                        ventana_ms,
                    }
                } else {
                    Vigilancia::Vivo {
                        identidad: identidad.clone(),
                        hace_ms,
                    }
                }
            })
            .collect();

        salida.extend(
            self.censo
                .iter()
                .filter(|identidad| !self.sensores.contains_key(*identidad))
                .map(|identidad| Vigilancia::NuncaVisto {
                    identidad: identidad.clone(),
                }),
        );

        salida
    }

    /// Condiciones activas que declaro un sensor en su ultimo latido.
    ///
    /// `None` si nunca hablo, que **no** es lo mismo que una lista vacia: la
    /// lista vacia dice que se miraron y no habia ninguna.
    #[must_use]
    pub fn condiciones_de(&self, identidad: &Identidad) -> Option<&[String]> {
        self.sensores
            .get(identidad)
            .map(|sensor| sensor.condiciones.as_slice())
    }
}

#[cfg(test)]
mod pruebas {
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    const MINUTO: i64 = 60_000;

    fn linea(maquina: &str, contador: u64, condiciones: &str) -> String {
        format!(
            "<110>1 2026-08-13T10:00:00.000Z {maquina} eje-agente - latido-de-sensor - \
             latido={contador} interfaz=eth0 sello=abc123 asiento=42 intervaloMs=60000 \
             condiciones={condiciones}"
        )
    }

    fn latido(maquina: &str, contador: u64) -> LatidoRecibido {
        analizar(&linea(maquina, contador, "ninguna")).expect("linea valida")
    }

    /// La identidad que produce `linea`, para comparar sin repetir la notacion.
    fn quien(maquina: &str) -> Identidad {
        Identidad::nueva(maquina, Some("eth0"))
    }

    // -----------------------------------------------------------------------
    // Lectura de la linea
    // -----------------------------------------------------------------------

    #[test]
    fn un_latido_se_lee_entero() {
        let leido = latido("sensor-uci", 7);

        assert_eq!(leido.maquina, "sensor-uci");
        assert_eq!(leido.interfaz.as_deref(), Some("eth0"));
        assert_eq!(leido.identidad(), quien("sensor-uci"));
        assert_eq!(leido.contador, 7);
        assert_eq!(leido.asiento, 42);
        assert_eq!(leido.sello, "abc123");
        assert_eq!(leido.intervalo_ms, 60_000);
        assert!(leido.condiciones.is_empty());
    }

    #[test]
    fn ninguna_es_una_afirmacion_y_no_una_ausencia_de_dato() {
        // La lista vacia dice «se miraron las ocho y no habia ninguna activa».
        // Que falte el campo entero es otra cosa y se rechaza.
        assert!(latido("s", 1).condiciones.is_empty());

        let con_estado = analizar(&linea("s", 1, "capturaNoDisponible,evidenciaEnRiesgo"))
            .expect("linea valida");
        assert_eq!(
            con_estado.condiciones,
            vec!["capturaNoDisponible", "evidenciaEnRiesgo"]
        );
    }

    #[test]
    fn lo_que_no_es_un_latido_no_se_confunde_con_uno() {
        // El sello lleva `sello=` y `asiento=` igual que el latido: distinguirlos
        // por esos campos es el error que ya se cometio en las pruebas del ciclo.
        let sello = "<110>1 2026-08-13T10:00:00.000Z s eje-agente - sello-de-evidencia - \
                     sello=abc123 asiento=42";
        assert_eq!(analizar(sello), None);

        let alerta = "<105>1 2026-08-13T10:00:00.000Z s eje-agente - amenaza-incontenible - \
                      asiento=1 dispositivo=00:1b:21 detalle";
        assert_eq!(analizar(alerta), None);

        assert_eq!(analizar(""), None);
        assert_eq!(analizar("cualquier cosa"), None);
    }

    #[test]
    fn un_latido_al_que_le_falta_un_campo_se_descarta_entero() {
        // Falla cerrado. Un latido a medias podria fijar una linea base falsa, y
        // eso compra silencio; ignorarlo acaba disparando la ausencia, que es la
        // respuesta segura.
        let sin_contador = "<110>1 2026-08-13T10:00:00.000Z s eje-agente - latido-de-sensor - \
                            interfaz=eth0 sello=abc asiento=1 intervaloMs=60000 \
                            condiciones=ninguna";
        assert_eq!(analizar(sin_contador), None);

        let sin_intervalo = "<110>1 2026-08-13T10:00:00.000Z s eje-agente - latido-de-sensor - \
                             latido=1 sello=abc asiento=1 condiciones=ninguna";
        assert_eq!(analizar(sin_intervalo), None);
    }

    #[test]
    fn un_intervalo_absurdo_no_se_corrige_se_descarta() {
        // Con intervalo cero la ventana es cero y el sensor estaria ausente en
        // todo momento; con uno negativo, nunca. Las dos son peores que no leerlo.
        let cero = "<110>1 2026-08-13T10:00:00.000Z s eje-agente - latido-de-sensor - \
                    latido=1 sello=abc asiento=1 intervaloMs=0 condiciones=ninguna";
        assert_eq!(analizar(cero), None);
    }

    #[test]
    fn un_campo_desconocido_no_invalida_el_latido() {
        // El agente puede ganar campos sin que este detector deje de servir. Lo
        // que no se tolera es que falte uno de los que se usan.
        let futuro = "<110>1 2026-08-13T10:00:00.000Z s eje-agente - latido-de-sensor - \
                      latido=1 sello=abc asiento=1 intervaloMs=60000 condiciones=ninguna \
                      algoNuevo=7";
        assert!(analizar(futuro).is_some());
    }

    // -----------------------------------------------------------------------
    // La maquina de estados
    // -----------------------------------------------------------------------

    #[test]
    fn el_primer_latido_establece_linea_base_y_no_afirma_nada_mas() {
        let mut vigia = Vigia::nuevo();

        assert_eq!(
            vigia.observar(&latido("sensor-uci", 1), 0),
            Acogida::LineaBase
        );
        assert_eq!(
            vigia.revisar(0),
            vec![Vigilancia::Vivo {
                identidad: quien("sensor-uci"),
                hace_ms: 0
            }]
        );
    }

    #[test]
    fn un_sensor_en_calma_late_igual_y_eso_no_es_sospechoso() {
        // LA prueba de la correccion a RPT-052 §4. Sin alertas nuevas el registro
        // no crece: `asiento` y `sello` son identicos latido tras latido. Tomar
        // eso por repeticion marcaria como sospechoso a todo sensor tranquilo,
        // que es precisamente el caso para el que existe el latido.
        let mut vigia = Vigia::nuevo();

        for numero in 1..=5 {
            let acogida = vigia.observar(&latido("sensor-calma", numero), numero as i64 * MINUTO);
            if numero > 1 {
                assert_eq!(acogida, Acogida::Continua, "latido {numero}");
            }
        }
    }

    #[test]
    fn un_latido_repetido_tal_cual_se_reconoce() {
        // Lo que el contador si compra: reproducir el mismo paquete trae un
        // numero ya visto.
        let mut vigia = Vigia::nuevo();

        vigia.observar(&latido("s", 9), 0);
        assert_eq!(
            vigia.observar(&latido("s", 9), MINUTO),
            Acogida::ReinicioORepeticion {
                visto: 9,
                recibido: 9
            }
        );
    }

    #[test]
    fn un_reinicio_no_se_declara_ataque_ni_el_ataque_se_declara_reinicio() {
        // El contador no sobrevive al proceso, asi que un reinicio legitimo
        // retrocede. Tiene la misma forma que una repeticion y el detector NO
        // elige: las declara juntas y decide un humano.
        let mut vigia = Vigia::nuevo();

        vigia.observar(&latido("s", 500), 0);

        assert_eq!(
            vigia.observar(&latido("s", 1), MINUTO),
            Acogida::ReinicioORepeticion {
                visto: 500,
                recibido: 1
            }
        );
    }

    #[test]
    fn un_hueco_en_la_serie_dice_cuantos_faltaron() {
        // La serie es contigua por construccion: el agente solo consume el numero
        // tras un envio correcto. Un salto significa que el canal se comio lineas.
        let mut vigia = Vigia::nuevo();

        vigia.observar(&latido("s", 1), 0);
        assert_eq!(
            vigia.observar(&latido("s", 4), MINUTO),
            Acogida::HuecoEnLaSerie { perdidos: 2 }
        );
    }

    // -----------------------------------------------------------------------
    // La ausencia — el motivo entero de este crate
    // -----------------------------------------------------------------------

    #[test]
    fn un_sensor_que_deja_de_latir_se_declara_ausente() {
        // Apagar el sensor y que la sala se entere. Es la condicion de cierre de
        // PA-104 (RPT-052 §6), aqui en su forma comprobable.
        let mut vigia = Vigia::nuevo();
        vigia.observar(&latido("sensor-uci", 1), 0);

        // Dentro de la ventana, sigue vivo.
        assert!(matches!(
            vigia.revisar(2 * MINUTO).as_slice(),
            [Vigilancia::Vivo { .. }]
        ));

        // Pasada la ventana, ausente.
        let despues = vigia.revisar(4 * MINUTO);
        assert_eq!(
            despues,
            vec![Vigilancia::Ausente {
                identidad: quien("sensor-uci"),
                hace_ms: 4 * MINUTO,
                ventana_ms: 3 * MINUTO,
            }]
        );
    }

    #[test]
    fn el_borde_de_la_ventana_no_dispara_todavia() {
        // Justo en la ventana no es pasada la ventana. Un `>=` aqui convertiria
        // cada latido puntual en una alarma.
        let mut vigia = Vigia::nuevo();
        vigia.observar(&latido("s", 1), 0);

        assert!(matches!(
            vigia.revisar(TOLERANCIA_INTERVALOS * MINUTO).as_slice(),
            [Vigilancia::Vivo { .. }]
        ));
        assert!(matches!(
            vigia.revisar(TOLERANCIA_INTERVALOS * MINUTO + 1).as_slice(),
            [Vigilancia::Ausente { .. }]
        ));
    }

    #[test]
    fn el_sensor_ausente_vuelve_a_estar_vivo_cuando_late() {
        let mut vigia = Vigia::nuevo();
        vigia.observar(&latido("s", 1), 0);
        assert!(matches!(
            vigia.revisar(10 * MINUTO).as_slice(),
            [Vigilancia::Ausente { .. }]
        ));

        vigia.observar(&latido("s", 2), 10 * MINUTO);
        assert!(matches!(
            vigia.revisar(10 * MINUTO).as_slice(),
            [Vigilancia::Vivo { .. }]
        ));
    }

    #[test]
    fn la_ventana_sale_del_intervalo_que_declara_cada_sensor() {
        // No hay una cifra global: cada sensor dice cada cuanto late y el vigia
        // le cree. Es lo que permite que un sensor de laboratorio lata cada diez
        // segundos y uno de planta cada cinco minutos sin negociar nada.
        let lento = "<110>1 2026-08-13T10:00:00.000Z lento eje-agente - latido-de-sensor - \
                     latido=1 interfaz=eth0 sello=abc asiento=1 intervaloMs=600000 \
                     condiciones=ninguna";

        let mut vigia = Vigia::nuevo();
        vigia.observar(&analizar(lento).expect("valida"), 0);
        vigia.observar(&latido("rapido", 1), 0);

        // A los cinco minutos el rapido lleva dos ventanas de retraso y el lento
        // ni siquiera ha agotado la primera.
        let estados = vigia.revisar(5 * MINUTO);
        assert!(
            estados.iter().any(|estado| matches!(
                estado,
                Vigilancia::Ausente { identidad, .. } if *identidad == quien("rapido")
            )),
            "{estados:?}"
        );
        assert!(
            estados.iter().any(|estado| matches!(
                estado,
                Vigilancia::Vivo { identidad, .. } if *identidad == quien("lento")
            )),
            "{estados:?}"
        );
    }

    // -----------------------------------------------------------------------
    // La identidad compuesta — RPT-059, PA-113
    // -----------------------------------------------------------------------

    #[test]
    fn dos_agentes_en_la_misma_maquina_son_dos_sensores() {
        // LA prueba de PA-113. Un servidor perimetral con un agente por segmento
        // comparte `HOSTNAME`. Con la maquina como clave unica, el latido de uno
        // taparia la muerte del otro y la sala no notaria nada.
        let uno = "<110>1 2026-08-13T10:00:00.000Z perimetro eje-agente - latido-de-sensor - \
                   latido=1 interfaz=eth0 sello=abc asiento=1 intervaloMs=60000 \
                   condiciones=ninguna";
        let otro = "<110>1 2026-08-13T10:00:00.000Z perimetro eje-agente - latido-de-sensor - \
                    latido=1 interfaz=eth1 sello=abc asiento=1 intervaloMs=60000 \
                    condiciones=ninguna";

        let mut vigia = Vigia::nuevo();
        assert_eq!(
            vigia.observar(&analizar(uno).expect("valida"), 0),
            Acogida::LineaBase
        );
        assert_eq!(
            vigia.observar(&analizar(otro).expect("valida"), 0),
            Acogida::LineaBase,
            "el segundo es un sensor nuevo, no la continuacion del primero"
        );

        assert_eq!(vigia.revisar(0).len(), 2, "dos sensores, no uno");
    }

    #[test]
    fn el_que_sigue_latiendo_no_tapa_la_muerte_del_companero() {
        // La consecuencia que importa, y la que se comprueba en la prueba de
        // fuego: uno se apaga y el otro sigue. Con identidad por maquina, el
        // segundo refrescaba el reloj del primero y la ausencia nunca saltaba.
        let vivo = "<110>1 2026-08-13T10:00:00.000Z perimetro eje-agente - latido-de-sensor - \
                    latido=9 interfaz=eth1 sello=abc asiento=1 intervaloMs=60000 \
                    condiciones=ninguna";

        let mut vigia = Vigia::nuevo();
        vigia.observar(
            &analizar(
                "<110>1 2026-08-13T10:00:00.000Z perimetro eje-agente - latido-de-sensor - \
                 latido=1 interfaz=eth0 sello=abc asiento=1 intervaloMs=60000 \
                 condiciones=ninguna",
            )
            .expect("valida"),
            0,
        );

        // eth1 sigue latiendo cuatro minutos despues; eth0 lleva callado desde
        // el principio.
        vigia.observar(&analizar(vivo).expect("valida"), 4 * MINUTO);

        let estados = vigia.revisar(4 * MINUTO);
        assert!(
            estados.iter().any(|estado| matches!(
                estado,
                Vigilancia::Ausente { identidad, .. }
                    if identidad.interfaz.as_deref() == Some("eth0")
            )),
            "{estados:?}"
        );
        assert!(
            estados.iter().any(|estado| matches!(
                estado,
                Vigilancia::Vivo { identidad, .. }
                    if identidad.interfaz.as_deref() == Some("eth1")
            )),
            "{estados:?}"
        );
    }

    #[test]
    fn un_agente_que_no_declara_interfaz_se_vigila_igual() {
        // Compatibilidad con lo anterior a RPT-059. Su identidad es la maquina
        // sola: distinta de cualquier par con interfaz, y estable. Descartar su
        // latido lo dejaria fuera de la vigilancia, que es lo contrario de lo
        // que se busca.
        let antiguo = "<110>1 2026-08-13T10:00:00.000Z viejo eje-agente - latido-de-sensor - \
                       latido=1 sello=abc asiento=1 intervaloMs=60000 condiciones=ninguna";

        let leido = analizar(antiguo).expect("sigue siendo un latido valido");
        assert_eq!(leido.interfaz, None);
        assert_eq!(leido.identidad(), Identidad::nueva("viejo", None));

        let mut vigia = Vigia::nuevo();
        vigia.observar(&leido, 0);
        assert!(matches!(
            vigia.revisar(10 * MINUTO).as_slice(),
            [Vigilancia::Ausente { .. }]
        ));
    }

    #[test]
    fn la_identidad_se_lee_y_se_escribe_con_la_misma_notacion() {
        // Si el censo se escribiera de una forma y el vigia imprimiera de otra,
        // habria entradas que nunca casan y se leerian como «ese sensor no ha
        // hablado». Es el defecto de RPT-058 §2 en su forma de notacion.
        for texto in ["maquina/eth0", "maquina"] {
            assert_eq!(Identidad::desde_texto(texto).to_string(), texto);
        }

        assert_eq!(
            Identidad::desde_texto("maquina/eth0"),
            Identidad::nueva("maquina", Some("eth0"))
        );
        assert_eq!(
            Identidad::desde_texto("maquina"),
            Identidad::nueva("maquina", None)
        );
        assert_ne!(
            Identidad::desde_texto("maquina"),
            Identidad::desde_texto("maquina/eth0"),
            "la maquina sola no es la maquina con interfaz"
        );
    }

    // -----------------------------------------------------------------------
    // El agujero que el censo tapa
    // -----------------------------------------------------------------------

    #[test]
    fn un_sensor_del_censo_que_nunca_hablo_no_pasa_por_inexistente() {
        // Sin esto, un sensor que se instalo y nunca arranco es invisible: no hay
        // ausencia donde no hubo presencia. Es «no se sabe», no «no hay».
        let mut vigia = Vigia::nuevo();
        vigia.esperar("sensor-quirofano");

        assert_eq!(
            vigia.revisar(0),
            vec![Vigilancia::NuncaVisto {
                identidad: Identidad::nueva("sensor-quirofano", None)
            }]
        );
    }

    #[test]
    fn nunca_visto_deja_de_serlo_en_cuanto_habla() {
        let mut vigia = Vigia::nuevo();
        // El censo se nombra igual que se imprime: `maquina/interfaz`. Nombrarlo
        // solo por la maquina dejaria una entrada que nunca casa, que es
        // exactamente lo que oculto el defecto de RPT-058 §2.
        vigia.esperar("s/eth0");
        vigia.observar(&latido("s", 1), 0);

        assert_eq!(
            vigia.revisar(0),
            vec![Vigilancia::Vivo {
                identidad: quien("s"),
                hace_ms: 0
            }],
            "no puede estar en las dos listas a la vez"
        );
    }

    #[test]
    fn un_sensor_fuera_del_censo_tambien_se_vigila_en_cuanto_habla() {
        // El censo anade cobertura; no la quita. Un sensor que aparece sin estar
        // declarado se vigila igual, porque callarse sobre el seria elegir no
        // mirar lo que ya se esta viendo.
        let mut vigia = Vigia::nuevo();
        vigia.observar(&latido("no-declarado", 1), 0);

        assert!(matches!(
            vigia.revisar(10 * MINUTO).as_slice(),
            [Vigilancia::Ausente { .. }]
        ));
    }

    // -----------------------------------------------------------------------
    // El estado que viaja con el latido
    // -----------------------------------------------------------------------

    #[test]
    fn la_sala_ve_las_condiciones_del_sensor_sin_preguntarle() {
        // RPT-052 §3: el latido lleva el estado precisamente para que la sala no
        // tenga que pedirlo por un camino que no tiene.
        let mut vigia = Vigia::nuevo();
        vigia.observar(
            &analizar(&linea("s", 1, "capturaNoDisponible")).expect("valida"),
            0,
        );

        assert_eq!(
            vigia.condiciones_de(&quien("s")),
            Some(["capturaNoDisponible".to_owned()].as_slice())
        );
    }

    #[test]
    fn de_un_sensor_que_nunca_hablo_no_se_sabe_nada_y_no_se_finge_calma() {
        // `None` y no lista vacia: la lista vacia afirma que se miraron y no
        // habia ninguna. De quien no ha hablado no se puede afirmar eso.
        let vigia = Vigia::nuevo();
        assert_eq!(vigia.condiciones_de(&quien("fantasma")), None);
    }

    #[test]
    fn un_sensor_ciego_sigue_contando_como_vivo() {
        // La diferencia entera de RPT-052 §4: un sensor que se apago y uno que
        // dejo de ver son dos llamadas distintas. El vigia no debe declararlo
        // ausente por estar degradado — lo esta diciendo el mismo.
        let mut vigia = Vigia::nuevo();
        vigia.observar(
            &analizar(&linea("s", 1, "capturaNoDisponible")).expect("valida"),
            0,
        );

        assert!(matches!(
            vigia.revisar(MINUTO).as_slice(),
            [Vigilancia::Vivo { .. }]
        ));
    }
}
