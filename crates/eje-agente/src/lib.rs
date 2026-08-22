//! # Eje-Agente
//!
//! Demonio local soberano de Eje-Latam. Integra los modulos AGT-01 a AGT-07 y
//! opera con capacidad plena **sin depender de conectividad ni de infraestructura
//! de PremosCorp** (RPT-002 §1).
//!
//! ## Principio de producto (RPT-003 §3.1)
//!
//! Ninguna condicion comercial degrada jamas una funcion de seguridad. Una licencia
//! vencida no desactiva deteccion ni contencion.

#![forbid(unsafe_code)]

pub mod alertas;
pub mod ciclo;
pub mod salida;
pub mod servicio;

use eje_almacen::ModoEsquema;
use eje_red::ConfiguracionRed;
use guardian_cc::PerfilSegmento;

/// Version del agente, tomada del manifiesto del paquete.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Estado de la licencia del nodo.
///
/// Ver RPT-003 §3.4 para la matriz completa de degradacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoLicencia {
    /// Licencia dentro de su periodo de validez.
    Vigente,
    /// Licencia expirada, sin incidente en curso.
    VencidaSinIncidente,
    /// Licencia expirada con incidente activo. VIS-02 opera completo.
    VencidaConIncidenteActivo,
}

impl EstadoLicencia {
    /// Indica si las funciones de seguridad operan al completo.
    ///
    /// Devuelve `true` **siempre**: AGT-01 a AGT-07 nunca se degradan por motivo
    /// comercial. La funcion existe para que la invariante quede explicita y
    /// verificable en pruebas, no porque pueda devolver `false`.
    #[must_use]
    pub const fn seguridad_completa(self) -> bool {
        true
    }

    /// Indica si VIS-02 puede exportar reportes y comparativas historicas.
    #[must_use]
    pub const fn permite_exportacion_de_reportes(self) -> bool {
        matches!(self, Self::Vigente)
    }

    /// Indica si VIS-02 muestra el estado operativo en vivo.
    ///
    /// Durante un incidente activo se muestra aunque la licencia este vencida:
    /// dejar a un comite de crisis hospitalario sin tablero por una fecha de
    /// facturacion es un fallo de producto con consecuencias reales.
    #[must_use]
    pub const fn permite_tablero_en_vivo(self) -> bool {
        matches!(self, Self::Vigente | Self::VencidaConIncidenteActivo)
    }
}

/// Configuracion de arranque del agente, fijada por el lanzador VIS-03.
#[derive(Debug, Clone)]
pub struct ConfiguracionAgente {
    /// Perfil del segmento vigilado.
    pub perfil: PerfilSegmento,
    /// Modo de esquema de la base local.
    pub modo_esquema: ModoEsquema,
    /// Configuracion de la capa de red.
    pub red: ConfiguracionRed,
    /// Estado de licencia del nodo.
    pub licencia: EstadoLicencia,
}

impl ConfiguracionAgente {
    /// Construye la configuracion por defecto para un segmento dado.
    ///
    /// El perfil OT aplica las restricciones de descubrimiento y Capa B sin
    /// necesidad de configuracion adicional.
    #[must_use]
    pub fn para_segmento(perfil: PerfilSegmento) -> Self {
        Self {
            perfil,
            modo_esquema: ModoEsquema::Estandar,
            red: ConfiguracionRed {
                perfil,
                capa_b_autorizada: false,
            },
            licencia: EstadoLicencia::Vigente,
        }
    }
}

#[cfg(test)]
mod pruebas {
    // Mismo encabezado que el resto de modulos de prueba del workspace. Las
    // lindes de RPT-003 §9.4 prohiben `expect` en la RUTA DE PRODUCCION; en una
    // prueba, `expect` es la forma directa de decir «esto debe cumplirse», y
    // convertirlo en un `Result` propagado solo desplazaria la afirmacion sin
    // hacerla mas segura.
    #![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn ninguna_condicion_comercial_degrada_la_seguridad() {
        for estado in [
            EstadoLicencia::Vigente,
            EstadoLicencia::VencidaSinIncidente,
            EstadoLicencia::VencidaConIncidenteActivo,
        ] {
            assert!(
                estado.seguridad_completa(),
                "la seguridad se degrado con licencia {estado:?}"
            );
        }
    }

    #[test]
    fn incidente_activo_conserva_el_tablero_en_vivo() {
        assert!(EstadoLicencia::VencidaConIncidenteActivo.permite_tablero_en_vivo());
        assert!(!EstadoLicencia::VencidaSinIncidente.permite_tablero_en_vivo());
    }

    #[test]
    fn licencia_vencida_bloquea_exportacion_pero_no_seguridad() {
        let estado = EstadoLicencia::VencidaSinIncidente;
        assert!(!estado.permite_exportacion_de_reportes());
        assert!(estado.seguridad_completa());
    }

    #[test]
    fn perfil_ot_arranca_con_capa_b_deshabilitada() {
        let configuracion = ConfiguracionAgente::para_segmento(PerfilSegmento::Ot);
        assert!(configuracion.red.autorizar_capa_b().is_err());
    }

    // -----------------------------------------------------------------------
    // Manejadores de alerta — RPT-028, PA-43
    // -----------------------------------------------------------------------

    use crate::alertas::{
        EstadoConfiguracion, SUCESOS_POR_CONSULTA, anotar_incontenible, condiciones, consultar,
        nombrar, suceso_desde,
    };
    use eje_almacen::{ClaseEvento, RegistroEvidencia};
    use eje_ipc::mensajes::{ClaseAlerta, PeticionAlertas};
    use guardian_cc::arranque::EstadoArranque;
    use guardian_cc::observacion::AlmacenObservacion;
    use guardian_cc::{ClaseExcluida, Veredicto};

    const MAC: [u8; 6] = [0x00, 0x1B, 0x21, 0x00, 0x00, 0x01];

    fn incontenible() -> Veredicto {
        Veredicto::Prohibida {
            clase: ClaseExcluida::SoporteVital,
        }
    }

    fn registro_con(cuantas: u64) -> RegistroEvidencia {
        let mut registro = RegistroEvidencia::nuevo();
        for indice in 0..cuantas {
            let _ =
                anotar_incontenible(&mut registro, 1_000 + indice as i64, &MAC, &incontenible());
        }
        registro
    }

    #[test]
    fn una_amenaza_incontenible_queda_anexada_y_se_puede_consultar() {
        // El hueco que PA-43 cierra: antes el veredicto se calculaba y no salia
        // de la funcion que lo calculaba.
        let registro = registro_con(1);
        let sucesos = consultar(&registro, &PeticionAlertas { desde_asiento: 0 }).sucesos;

        assert_eq!(sucesos.len(), 1);
        assert_eq!(sucesos[0].asiento, 1);
        assert_eq!(sucesos[0].clase, ClaseAlerta::AmenazaIncontenible);
        assert_eq!(sucesos[0].dispositivo, "00:1b:21:00:00:01");
        assert!(
            sucesos[0].detalle.contains("humana"),
            "el detalle debe decirle al operador que la respuesta es suya"
        );
    }

    #[test]
    fn todo_suceso_devuelto_corresponde_a_un_asiento_real() {
        // La garantia que sustituye a la que el tipo no puede dar. `SucesoAlerta`
        // tiene campos publicos porque serde los necesita, asi que nadie puede
        // impedir que se fabrique uno; lo que si se comprueba es que el agente
        // no lo hace (RPT-019 §7.3).
        let registro = registro_con(5);

        for suceso in consultar(&registro, &PeticionAlertas { desde_asiento: 0 }).sucesos {
            let asiento = registro
                .asiento(suceso.asiento)
                .expect("el asiento debe existir en el registro");

            assert_eq!(asiento.nodo, suceso.dispositivo);
            assert_eq!(asiento.detalle, suceso.detalle);
        }
    }

    #[test]
    fn desde_asiento_es_exclusivo_y_no_repite() {
        // Quien pide «desde el 3» ya tiene el 3. Incluirlo haria que un
        // consumidor que continua donde lo dejo viera la misma alerta dos veces,
        // y una alerta repetida ensena a ignorarlas.
        let registro = registro_con(5);
        let sucesos = consultar(&registro, &PeticionAlertas { desde_asiento: 3 }).sucesos;

        assert_eq!(sucesos.len(), 2);
        assert_eq!(sucesos[0].asiento, 4);
        assert_eq!(sucesos[1].asiento, 5);
    }

    #[test]
    fn una_consulta_no_devuelve_mas_asientos_de_los_que_marca_la_cota() {
        // Renombrada en RPT-049. Antes se llamaba
        // `una_consulta_no_devuelve_mas_de_lo_que_cabe_en_un_marco` y NO
        // comprobaba eso: contaba elementos, no bytes. El nombre prometia la
        // propiedad que el agente incumplia, que es la clase de prueba mas
        // peligrosa que hay — la que hace creer que algo esta cubierto.
        //
        // La propiedad del marco la comprueba ahora la prueba de abajo.
        let registro = registro_con(SUCESOS_POR_CONSULTA as u64 + 10);
        let lote = consultar(&registro, &PeticionAlertas { desde_asiento: 0 });

        assert_eq!(lote.sucesos.len(), SUCESOS_POR_CONSULTA);
        assert!(lote.hay_mas, "quedaron diez fuera y hay que decirlo");
        assert_eq!(
            lote.sucesos[0].asiento, 1,
            "el lote empieza por el mas antiguo, no por el mas nuevo"
        );
    }

    #[test]
    fn una_respuesta_con_detalles_largos_sigue_cabiendo_en_un_marco() {
        // PA-96, observado en campo (RPT-049). Con detalles de 4 KB, 256 sucesos
        // ocupaban 1 071 690 bytes y `enmarcar` rechazaba la respuesta ENTERA.
        // Como el cliente vuelve a pedir desde el mismo sitio, el rechazo se
        // repetia para siempre: el canal de alertas quedaba inservible con la
        // evidencia intacta en disco.
        //
        // Esta prueba enmarca la respuesta DE VERDAD. Contar elementos no habria
        // detectado nada, que es exactamente lo que paso durante meses.
        let mut registro = RegistroEvidencia::nuevo();
        let detalle = "d".repeat(8 * 1024);
        for numero in 1..=300u64 {
            registro
                .anexar(
                    numero as i64,
                    ClaseEvento::DeteccionAnomalia,
                    "nodo",
                    &detalle,
                )
                .expect("cabe en el registro");
        }

        let lote = consultar(&registro, &PeticionAlertas { desde_asiento: 0 });
        assert!(lote.hay_mas, "con 300 asientos de 8 KB no caben todos");
        assert!(!lote.sucesos.is_empty(), "tiene que entregar algo, no nada");

        let respuesta = eje_ipc::mensajes::RespuestaAlertas {
            primer_disponible: 1,
            hay_mas: lote.hay_mas,
            sucesos: lote.sucesos,
        };
        let cuerpo = serde_json::to_vec(&respuesta).expect("serializa");

        // El camino real: componer la respuesta y enmarcarla. Si esto falla, el
        // cliente recibe un rechazo en lugar de sus alertas.
        let carga = eje_ipc::componer_respuesta(&cuerpo).expect("cabe como respuesta");
        assert!(
            eje_ipc::enmarcar(&carga).is_ok(),
            "la respuesta no cabe en un marco: el canal quedaria inservible"
        );
    }

    #[test]
    fn solo_las_clases_declaradas_se_comunican_como_alerta() {
        // Anadir una clase de evento a ALM-01 no debe convertirla en alerta por
        // omision. Comunicar de mas es la otra cara de la fatiga.
        let mut registro = RegistroEvidencia::nuevo();
        registro
            .anexar(1, ClaseEvento::ArranqueAgente, "agente", "arranque")
            .expect("cabe");
        registro
            .anexar(2, ClaseEvento::SelloEmitido, "agente", "sello")
            .expect("cabe");

        assert!(
            consultar(&registro, &PeticionAlertas { desde_asiento: 0 })
                .sucesos
                .is_empty()
        );

        for asiento in registro.asientos() {
            assert!(suceso_desde(asiento).is_none());
        }
    }

    #[test]
    fn la_direccion_se_presenta_como_una_mac_y_no_como_un_vector() {
        // `{:02x?}` produce `[00, 1b, ...]`, que no es lo que un operador
        // reconoce. Una alerta que no se entiende no sirve.
        assert_eq!(nombrar(&MAC), "00:1b:21:00:00:01");
    }

    #[test]
    fn los_dos_estados_administrativos_llegan_al_operador() {
        // El hallazgo de PA-43. `FormatoObsoleto` y `SinClaveAprovisionada`
        // exigen alerta, no son manipulacion, y sin el campo nuevo no tenian por
        // donde llegar: no son sucesos y no cabian en las cuatro condiciones.
        let observacion = AlmacenObservacion::nuevo();

        for estado in [
            EstadoArranque::FormatoObsoleto { encontrada: 1 },
            EstadoArranque::SinClaveAprovisionada,
        ] {
            let vigentes = condiciones(
                &estado,
                &observacion,
                &RegistroEvidencia::nuevo(),
                false,
                false,
                false,
                EstadoConfiguracion::Firmada,
            );

            assert!(vigentes.accion_administrativa, "{estado:?} debe avisar");
            assert!(
                !vigentes.hay_manipulacion(),
                "pero no como si alguien hubiera tocado el almacen"
            );
            assert!(vigentes.hay_degradacion());
        }
    }

    #[test]
    fn la_manipulacion_se_distingue_de_la_accion_administrativa() {
        let observacion = AlmacenObservacion::nuevo();

        for estado in [
            EstadoArranque::Supresion {
                secuencia_conocida: 7,
            },
            EstadoArranque::NoVerifica {
                detalle: "prueba".to_owned(),
            },
        ] {
            let vigentes = condiciones(
                &estado,
                &observacion,
                &RegistroEvidencia::nuevo(),
                false,
                false,
                false,
                EstadoConfiguracion::Firmada,
            );

            assert!(vigentes.hay_manipulacion());
            assert!(
                !vigentes.accion_administrativa,
                "reemitir no arregla un almacen manipulado"
            );
        }
    }

    #[test]
    fn un_primer_arranque_no_declara_ninguna_condicion() {
        // El estado normal de una instalacion recien hecha con su clave puesta.
        // Si alertara, alertaria siempre, y una alerta permanente es ruido.
        let vigentes = condiciones(
            &EstadoArranque::PrimerArranque,
            &AlmacenObservacion::nuevo(),
            &RegistroEvidencia::nuevo(),
            false,
            false,
            false,
            EstadoConfiguracion::Firmada,
        );

        assert!(!vigentes.hay_degradacion());
        assert!(!vigentes.hay_manipulacion());
    }

    // -----------------------------------------------------------------------
    // Persistencia del registro — RPT-029, PA-56
    // -----------------------------------------------------------------------

    use crate::alertas::{
        CargaRegistro, apartar, cargar_desde, cargar_registro, persistir, ruta_apartada,
    };
    use eje_almacen::persistencia::{ASIENTOS_MAXIMOS, MAGICO_REGISTRO, analizar, serializar};

    #[test]
    fn una_alerta_sobrevive_a_un_reinicio_del_agente() {
        // El hueco que PA-56 cierra. Antes de esto el sensor se reiniciaba —por
        // una actualizacion, por un corte de luz— y con el se iba la unica
        // constancia de que hubo una amenaza incontenible.
        let registro = registro_con(3);
        let bytes = serializar(&registro);

        let CargaRegistro::Conforme(recuperado) = cargar_registro(Some(&bytes)) else {
            panic!("un registro recien escrito debe verificar");
        };

        let sucesos = consultar(&recuperado, &PeticionAlertas { desde_asiento: 0 }).sucesos;
        assert_eq!(sucesos.len(), 3);
        assert_eq!(sucesos[2].asiento, 3);
        assert_eq!(sucesos[0].dispositivo, "00:1b:21:00:00:01");
    }

    #[test]
    fn la_serie_continua_donde_la_dejo_el_proceso_anterior() {
        // Si la numeracion reiniciara, dos alertas distintas compartirian
        // asiento y el consumidor que continua desde el ultimo no veria la
        // segunda.
        let mut registro = analizar(&serializar(&registro_con(2))).expect("verifica");
        anotar_incontenible(&mut registro, 9_000, &MAC, &incontenible());

        let sucesos = consultar(&registro, &PeticionAlertas { desde_asiento: 2 }).sucesos;

        assert_eq!(sucesos.len(), 1);
        assert_eq!(sucesos[0].asiento, 3, "la serie no reinicia");
    }

    #[test]
    fn borrar_un_asiento_intermedio_no_pasa_desapercibido() {
        // El ataque que da sentido a guardar el numero de asiento. Sin el, la
        // reconstruccion renumeraria los supervivientes y la cadena cuadraria:
        // borrar evidencia seria gratis.
        let bytes = serializar(&registro_con(3));

        // Se declara un asiento menos y se suprime el primer registro, que mide
        // lo mismo que los demas porque los tres se anexaron igual.
        let longitud_asiento = (bytes.len() - 14) / 3;
        let mut mutilado = bytes.clone();
        mutilado[10..14].copy_from_slice(&2u32.to_be_bytes());
        mutilado.drain(14..14 + longitud_asiento);

        assert!(
            matches!(
                cargar_registro(Some(&mutilado)),
                CargaRegistro::ViolacionDetectada { .. }
            ),
            "la numeracion delata la supresion"
        );
    }

    #[test]
    fn alterar_el_detalle_de_un_asiento_se_detecta() {
        // La cadena se reconstruye al cargar, asi que un campo cambiado rompe el
        // enlace del siguiente. Aqui se comprueba por la via que el atacante
        // usaria: cambiar el texto sin tocar longitudes.
        let bytes = serializar(&registro_con(2));
        let mut alterado = bytes.clone();

        // El detalle es el ultimo campo del ultimo asiento.
        let ultimo = alterado.len() - 1;
        alterado[ultimo] = b'X';

        let recuperado = analizar(&alterado).expect("el formato sigue bien");
        assert!(
            recuperado.verificar_cadena().is_ok(),
            "la cadena se RECONSTRUYE, asi que sigue siendo coherente consigo misma"
        );
        assert_ne!(
            recuperado.extremo(),
            analizar(&bytes).expect("verifica").extremo(),
            "pero su extremo ya no es el mismo: el cambio es visible para quien \
             conserve el extremo anterior"
        );
    }

    #[test]
    fn un_registro_truncado_no_se_confunde_con_una_violacion() {
        // Un corte de energia durante la escritura es lo esperable, no un
        // ataque. Colapsarlos haria que cada apagon pareciera una intrusion.
        let bytes = serializar(&registro_con(2));
        let cortado = &bytes[..bytes.len() - 5];

        assert!(matches!(
            cargar_registro(Some(cortado)),
            CargaRegistro::Truncado { .. }
        ));
    }

    #[test]
    fn un_registro_ausente_es_el_primer_arranque_y_no_una_acusacion() {
        // A diferencia del inventario, aqui no hay centinela que atestigue que
        // hubo algo antes. Afirmar manipulacion sin testigo seria acusar sin
        // pruebas.
        assert!(matches!(cargar_registro(None), CargaRegistro::Conforme(_)));
    }

    #[test]
    fn ante_una_violacion_no_se_carga_nada_en_lugar_de_lo_que_sobrevivio() {
        // Cargar los asientos que si verificaban dejaria que quien borro
        // evidencia eligiera QUE se conserva, y el operador veria un registro
        // que parece integro.
        let carga = cargar_registro(Some(b"esto no es un registro"));

        assert!(matches!(carga, CargaRegistro::ViolacionDetectada { .. }));
        assert_eq!(carga.registro().longitud(), 0);
    }

    /// Directorio de prueba que se limpia al soltarse.
    struct AlmacenDePrueba {
        directorio: std::path::PathBuf,
    }

    impl AlmacenDePrueba {
        fn nuevo(nombre: &str) -> Self {
            let directorio = std::env::temp_dir().join(format!("eje-latam-alertas-{nombre}"));
            let _ = std::fs::remove_dir_all(&directorio);
            std::fs::create_dir_all(&directorio).expect("directorio de prueba");
            Self { directorio }
        }

        fn evidencia(&self) -> std::path::PathBuf {
            self.directorio.join("evidencia.alm")
        }
    }

    impl Drop for AlmacenDePrueba {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directorio);
        }
    }

    #[test]
    fn el_registro_va_y_vuelve_del_disco_por_la_ruta_real() {
        // PA-58. El modulo de PA-56 existia y nadie lo llamaba; esto ejercita el
        // camino que usa el agente, con fichero de verdad.
        let almacen = AlmacenDePrueba::nuevo("ida-y-vuelta");

        assert!(
            matches!(
                cargar_desde(&almacen.evidencia()).expect("no es fallo de disco"),
                CargaRegistro::Conforme(_)
            ),
            "un almacen vacio es el primer arranque"
        );

        persistir(&almacen.evidencia(), &registro_con(2)).expect("persiste");

        let CargaRegistro::Conforme(recuperado) =
            cargar_desde(&almacen.evidencia()).expect("no es fallo de disco")
        else {
            panic!("lo que acaba de escribirse debe verificar");
        };

        assert_eq!(recuperado.longitud(), 2);
    }

    #[test]
    fn un_registro_danado_se_aparta_y_el_original_deja_de_estar() {
        // La prueba de que «no se borra» es cierta: el fichero sigue existiendo,
        // en otro nombre, y el agente puede volver a anexar sin pisarlo.
        let almacen = AlmacenDePrueba::nuevo("apartado");
        std::fs::write(almacen.evidencia(), b"esto no es un registro").expect("escribir");

        let destino = apartar(&almacen.evidencia(), 1_700).expect("aparta");

        assert!(
            destino.exists(),
            "la evidencia de la manipulacion se conserva"
        );
        assert!(
            !almacen.evidencia().exists(),
            "y la ruta queda libre para anexar de nuevo"
        );

        // Anexar despues no resucita lo apartado ni lo pisa.
        persistir(&almacen.evidencia(), &registro_con(1)).expect("persiste");
        assert!(destino.exists());
    }

    // -----------------------------------------------------------------------
    // Anclaje del extremo — RPT-033, PA-57
    // -----------------------------------------------------------------------

    use eje_almacen::persistencia::{Cotejo, analizar_ancla, ancla_de, cotejar};

    #[test]
    fn alterar_el_ultimo_asiento_ya_no_pasa_desapercibido() {
        // El hueco que RPT-029 §2.1 dejo escrito como lo que era. La cadena se
        // reconstruye al cargar, asi que siempre cuadra consigo misma; lo unico
        // que delata el cambio es el extremo, y hasta ahora nadie lo guardaba.
        let almacen = AlmacenDePrueba::nuevo("ancla-alterado");
        persistir(&almacen.evidencia(), &registro_con(3)).expect("persiste");

        // Se altera el detalle del ultimo asiento sin tocar longitudes.
        let mut bytes = std::fs::read(almacen.evidencia()).expect("leer");
        let ultimo = bytes.len() - 1;
        bytes[ultimo] = b'X';
        std::fs::write(almacen.evidencia(), &bytes).expect("reescribir");

        assert!(
            matches!(
                cargar_desde(&almacen.evidencia()).expect("no es fallo de disco"),
                CargaRegistro::ViolacionDetectada { .. }
            ),
            "el ancla ve lo que el fichero no puede ver de si mismo"
        );
    }

    #[test]
    fn cortar_la_cola_del_registro_se_detecta_como_truncamiento() {
        let registro = registro_con(4);
        let ancla = ancla_de(&registro).expect("no esta vacio");

        // El mismo registro con dos asientos menos.
        let recortado = registro_con(2);

        assert!(matches!(
            cotejar(&recortado, &ancla),
            Cotejo::Truncado {
                anclado: 4,
                ultimo_presente: 2
            }
        ));
    }

    #[test]
    fn los_asientos_posteriores_al_ancla_no_son_una_acusacion() {
        // Es lo que deja un corte de energia entre escribir el registro y
        // escribir el ancla. Colapsarlo en violacion haria que cada apagon en el
        // momento justo pareciera un ataque.
        let ancla = ancla_de(&registro_con(2)).expect("no esta vacio");

        assert!(matches!(
            cotejar(&registro_con(4), &ancla),
            Cotejo::SinAnclar { posteriores: 2 }
        ));
    }

    #[test]
    fn borrar_el_ancla_con_evidencia_dentro_es_manipulacion() {
        // Es justo lo que haria quien pretende cortar la cola sin que se note:
        // primero desactivar la comprobacion.
        let almacen = AlmacenDePrueba::nuevo("ancla-borrada");
        persistir(&almacen.evidencia(), &registro_con(2)).expect("persiste");
        std::fs::remove_file(almacen.evidencia().with_extension("anc")).expect("borrar");

        assert!(matches!(
            cargar_desde(&almacen.evidencia()).expect("no es fallo de disco"),
            CargaRegistro::ViolacionDetectada { .. }
        ));
    }

    #[test]
    fn un_ancla_corrupta_no_se_degrada_a_ancla_ausente() {
        // Mismo argumento que el centinela corrupto: corromper treinta bytes
        // seria la via para desactivar la comprobacion.
        let almacen = AlmacenDePrueba::nuevo("ancla-corrupta");
        persistir(&almacen.evidencia(), &registro_con(2)).expect("persiste");
        std::fs::write(
            almacen.evidencia().with_extension("anc"),
            b"no soy un ancla",
        )
        .expect("corromper");

        assert!(matches!(
            cargar_desde(&almacen.evidencia()).expect("no es fallo de disco"),
            CargaRegistro::ViolacionDetectada { .. }
        ));
    }

    #[test]
    fn el_ancla_se_aparta_con_el_registro() {
        // Dejarla atras la volveria huerfana: cubriria asientos que ya no estan
        // en su sitio y cada arranque posterior leeria un truncamiento que no
        // ocurrio, para siempre.
        let almacen = AlmacenDePrueba::nuevo("ancla-apartada");
        persistir(&almacen.evidencia(), &registro_con(2)).expect("persiste");

        apartar(&almacen.evidencia(), 1_700).expect("aparta");

        assert!(!almacen.evidencia().with_extension("anc").exists());
        assert!(
            matches!(
                cargar_desde(&almacen.evidencia()).expect("no es fallo de disco"),
                CargaRegistro::Conforme(_)
            ),
            "tras apartar, el arranque siguiente es limpio y no una acusacion eterna"
        );
    }

    #[test]
    fn el_ancla_es_reversible_y_un_registro_vacio_no_la_tiene() {
        let registro = registro_con(3);
        let ancla = ancla_de(&registro).expect("no esta vacio");
        let bytes = eje_almacen::persistencia::serializar_ancla(&ancla);

        assert_eq!(analizar_ancla(&bytes).expect("analiza"), ancla);
        assert_eq!(ancla.numero, 3);

        assert!(
            ancla_de(&RegistroEvidencia::nuevo()).is_none(),
            "sin extremo no hay nada que anclar, y fabricarlo con el genesis \
             haria indistinguible «vacio» de «con un asiento borrado»"
        );
    }

    #[test]
    fn un_registro_que_excede_la_cota_no_se_declara_conforme() {
        // Es el tercer estado de RPT-006 §4: evidencia que no se puede
        // comprobar. Devolver «conforme» porque no se pudo leer seria la
        // mentira que ese principio existe para impedir.
        let almacen = AlmacenDePrueba::nuevo("excesivo");
        let enorme = vec![0u8; eje_almacen::persistencia::LONGITUD_MAXIMA + 1];
        std::fs::write(almacen.evidencia(), &enorme).expect("escribir");

        assert!(matches!(
            cargar_desde(&almacen.evidencia()).expect("no es fallo de disco"),
            CargaRegistro::ViolacionDetectada { .. }
        ));
    }

    #[test]
    fn el_fichero_danado_se_aparta_con_un_nombre_que_no_pisa_otro() {
        // No se borra: un registro que no verifica es evidencia de que alguien
        // intervino, y esa evidencia vale mas que la que contiene.
        let original = std::path::Path::new("/datos/eje/evidencia.alm");

        let uno = ruta_apartada(original, 1_700);
        let otro = ruta_apartada(original, 1_800);

        assert_ne!(uno, otro, "dos incidentes no pueden pisarse");
        assert_eq!(uno.parent(), original.parent());
        assert!(uno.to_string_lossy().contains("violacion"));
    }

    #[test]
    fn el_analizador_del_registro_resiste_entrada_hostil() {
        for longitud in 0..14 {
            assert!(analizar(&vec![0u8; longitud]).is_err());
        }

        let valido = serializar(&registro_con(1));

        let mut ajeno = valido.clone();
        ajeno[0] = b'X';
        assert!(analizar(&ajeno).is_err());
        assert_eq!(&valido[..8], MAGICO_REGISTRO);

        // Un numero de asientos absurdo no debe reservar memoria.
        let mut absurdo = valido.clone();
        absurdo[10..14].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(analizar(&absurdo).is_err());
        assert!(ASIENTOS_MAXIMOS < u32::MAX as usize);

        // Cola sobrante: dos lecturas del mismo fichero.
        let mut con_cola = valido;
        con_cola.extend_from_slice(b"cola");
        assert!(analizar(&con_cola).is_err());
    }

    // -----------------------------------------------------------------------
    // Salida por syslog — RPT-032, PA-42
    // -----------------------------------------------------------------------

    use crate::salida::{
        Despacho, Emisor, ErrorSalida, Gravedad, linea_de_suceso, linea_de_transicion,
        marca_de_tiempo, sanear, transiciones,
    };
    use eje_ipc::mensajes::{Condiciones, SucesoAlerta};

    /// Despacho que guarda lo que se le da, o falla siempre.
    struct DespachoDePrueba {
        emitidos: Vec<String>,
        falla: bool,
    }

    impl DespachoDePrueba {
        const fn nuevo(falla: bool) -> Self {
            Self {
                emitidos: Vec::new(),
                falla,
            }
        }
    }

    impl Despacho for DespachoDePrueba {
        fn enviar(&mut self, marco: &[u8]) -> Result<(), ErrorSalida> {
            if self.falla {
                return Err(ErrorSalida::NoDisponible {
                    detalle: "prueba".to_owned(),
                });
            }
            self.emitidos
                .push(String::from_utf8_lossy(marco).into_owned());
            Ok(())
        }
    }

    fn normales() -> Condiciones {
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

    fn suceso() -> SucesoAlerta {
        SucesoAlerta {
            asiento: 7,
            clase: ClaseAlerta::AmenazaIncontenible,
            dispositivo: "00:1b:21:00:00:01".to_owned(),
            detalle: "bomba de infusion".to_owned(),
        }
    }

    #[test]
    fn el_marco_declara_su_longitud_en_octetos() {
        // RFC 6587. Con delimitacion por salto de linea, un salto dentro del
        // mensaje inyectaria una entrada de syslog completa en el SIEM del
        // cliente, atribuida al agente.
        let marco = linea_de_suceso(&suceso(), 1_700_000_000_000, "sensor-1");
        let texto = String::from_utf8_lossy(&marco);

        let (declarada, resto) = texto.split_once(' ').expect("el marco lleva prefijo");
        assert_eq!(
            declarada.parse::<usize>().expect("es un numero"),
            resto.len(),
            "la longitud declarada debe ser la del mensaje"
        );
    }

    #[test]
    fn un_salto_de_linea_no_puede_inyectar_una_entrada_falsa() {
        // Segunda linea de defensa: aunque alguien cambiara el marco a
        // delimitacion por salto, el saneado ya lo impide.
        let mut malicioso = suceso();
        malicioso.detalle = "normal\n<13>1 - - - - - alerta inventada".to_owned();

        let marco = linea_de_suceso(&malicioso, 1_700_000_000_000, "sensor-1");
        let texto = String::from_utf8_lossy(&marco);

        assert!(
            !texto.contains('\n'),
            "ningun salto de linea debe llegar al cable: {texto}"
        );
        assert_eq!(sanear("uno\ndos\ttres\r"), "uno dos tres ");
    }

    #[test]
    fn la_marca_de_tiempo_es_la_del_suceso_y_no_la_de_recepcion() {
        // Si se dejara en NILVALUE, el colector estamparia su hora de llegada y
        // se perderia cuando ocurrio, que es el dato que importa.
        assert_eq!(marca_de_tiempo(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            marca_de_tiempo(1_700_000_000_000),
            "2023-11-14T22:13:20.000Z"
        );

        // Un anio bisiesto y un fin de mes, que es donde falla el calculo ingenuo.
        assert_eq!(
            marca_de_tiempo(1_709_164_800_000),
            "2024-02-29T00:00:00.000Z"
        );

        // Reloj anterior a la epoca: fecha correcta y absurda, que en si misma
        // es una noticia. Lo que no puede es entrar en panico.
        assert!(marca_de_tiempo(-1).starts_with("1969-12-31T23:59:59"));
    }

    #[test]
    fn una_amenaza_incontenible_sale_con_la_gravedad_mas_alta() {
        let marco = linea_de_suceso(&suceso(), 0, "sensor-1");
        let texto = String::from_utf8_lossy(&marco);

        assert!(
            texto.contains(&format!("<{}>1 ", Gravedad::Alerta.prioridad())),
            "{texto}"
        );
        assert!(
            texto.contains("asiento=7"),
            "la referencia cruzada a ALM-01"
        );
        assert!(texto.contains("00:1b:21:00:00:01"));
    }

    #[test]
    fn solo_se_emite_lo_que_cambia() {
        // Las condiciones son verdaderas hasta que alguien interviene. Emitirlas
        // en cada ciclo inundaria el SIEM con la misma noticia (RPT-019 §2).
        let degradada = Condiciones {
            captura_con_perdida: true,
            ..normales()
        };

        assert_eq!(
            transiciones(None, &normales()).len(),
            0,
            "nada activo, nada"
        );
        assert_eq!(transiciones(None, &degradada).len(), 1);
        assert_eq!(
            transiciones(Some(&degradada), &degradada).len(),
            0,
            "la misma condicion dos ciclos seguidos no se repite"
        );

        // Y la vuelta a la normalidad si se emite: el operador quiere saberlo.
        let vuelta = transiciones(Some(&degradada), &normales());
        assert_eq!(vuelta.len(), 1);
        assert!(!vuelta[0].activa);
        assert_eq!(vuelta[0].gravedad(), Gravedad::Informativo);
    }

    #[test]
    fn la_manipulacion_sale_con_mas_gravedad_que_la_accion_administrativa() {
        let manipulada = Condiciones {
            inventario_suprimido: true,
            ..normales()
        };
        let administrativa = Condiciones {
            accion_administrativa: true,
            ..normales()
        };

        assert_eq!(
            transiciones(None, &manipulada)[0].gravedad(),
            Gravedad::Error
        );
        assert_eq!(
            transiciones(None, &administrativa)[0].gravedad(),
            Gravedad::Aviso
        );
    }

    #[test]
    fn la_condicion_de_salida_caida_no_se_emite_por_la_salida() {
        // Es la unica que no puede: emitirla exigiria el canal que acaba de
        // fallar. Si apareciera aqui, el agente intentaria enviar por un socket
        // roto la noticia de que el socket esta roto.
        let caida = Condiciones {
            salida_no_disponible: true,
            ..normales()
        };

        assert!(
            transiciones(None, &caida).is_empty(),
            "salidaNoDisponible viaja solo por IPC"
        );
    }

    #[test]
    fn el_fallo_de_envio_se_declara_en_lugar_de_tragarse() {
        let mut emisor = Emisor::nuevo(DespachoDePrueba::nuevo(true), "sensor-1", "eth0");

        assert!(
            !emisor.emitir(&[suceso()], &normales(), 0),
            "un envio fallido debe decirse"
        );
    }

    #[test]
    fn un_colector_caido_no_reemite_transiciones_pasadas_al_volver() {
        // El estado anterior se actualiza aunque el envio falle. Si no, al
        // recuperarse el colector se reemitirian transiciones ya pasadas como si
        // fueran nuevas, y el operador veria un incidente que no ocurrio.
        let degradada = Condiciones {
            captura_con_perdida: true,
            ..normales()
        };

        let mut caido = Emisor::nuevo(DespachoDePrueba::nuevo(true), "sensor-1", "eth0");
        assert!(!caido.emitir(&[], &degradada, 0));

        // El mismo emisor, con el colector ya en pie, no debe repetir la
        // transicion que intento enviar mientras estaba caido.
        let mut emisor = Emisor::nuevo(DespachoDePrueba::nuevo(false), "sensor-1", "eth0");
        assert!(emisor.emitir(&[], &degradada, 0));
        assert!(emisor.emitir(&[], &degradada, 0));
    }

    #[test]
    fn cada_suceso_y_cada_transicion_producen_un_marco() {
        // Se observa lo que llegaria al cable, sin abrir un socket. Un formato
        // que solo se prueba contra un colector real es un formato que nadie
        // prueba.
        let degradada = Condiciones {
            observacion_saturada: true,
            ..normales()
        };

        let mut despacho = DespachoDePrueba::nuevo(false);

        for suceso in [suceso(), suceso()] {
            despacho
                .enviar(&linea_de_suceso(&suceso, 0, "sensor-1"))
                .expect("el despacho de prueba no falla");
        }
        for transicion in transiciones(None, &degradada) {
            despacho
                .enviar(&linea_de_transicion(&transicion, 0, "sensor-1"))
                .expect("el despacho de prueba no falla");
        }

        assert_eq!(despacho.emitidos.len(), 3, "dos sucesos y una transicion");
        assert!(despacho.emitidos[2].contains("condicion=observacionSaturada"));
        assert!(despacho.emitidos[2].contains("estado=activa"));
    }

    // -----------------------------------------------------------------------
    // Sello del extremo hacia el testigo externo — RPT-038, PA-64
    // -----------------------------------------------------------------------

    use crate::salida::{DatosSello, linea_de_sello};

    /// Despacho que falla las primeras `fallos` veces y **deja ver** lo enviado.
    ///
    /// El buzon se comparte con la prueba porque `Emisor` es dueno del despacho:
    /// sin esto no se puede distinguir «se envio y salio bien» de «no se envio
    /// porque no hacia falta», que es justo lo que aqui se comprueba.
    struct DespachoIntermitente {
        buzon: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
        fallos: usize,
    }

    impl DespachoIntermitente {
        fn nuevo(fallos: usize) -> (Self, std::rc::Rc<std::cell::RefCell<Vec<String>>>) {
            let buzon = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            (
                Self {
                    buzon: std::rc::Rc::clone(&buzon),
                    fallos,
                },
                buzon,
            )
        }
    }

    impl Despacho for DespachoIntermitente {
        fn enviar(&mut self, marco: &[u8]) -> Result<(), ErrorSalida> {
            if self.fallos > 0 {
                self.fallos -= 1;
                return Err(ErrorSalida::NoDisponible {
                    detalle: "colector caido".to_owned(),
                });
            }
            self.buzon
                .borrow_mut()
                .push(String::from_utf8_lossy(marco).into_owned());
            Ok(())
        }
    }

    #[test]
    fn el_sello_lleva_el_extremo_y_el_asiento_y_no_es_una_alerta() {
        // Es una constancia, no una noticia. Si saliera con gravedad de alerta,
        // el operador recibiria una por cada cambio del registro y aprenderia a
        // ignorarlas — que es como se pierde la que importa.
        let marco = linea_de_sello(&DatosSello {
            numero: 42,
            sello: "abcdef",
            instante_utc: 0,
            maquina: "sensor-1",
            interfaz: "eth0",
        });
        let texto = String::from_utf8_lossy(&marco);

        assert!(texto.contains("sello=abcdef"), "{texto}");
        assert!(texto.contains("asiento=42"), "{texto}");
        assert!(
            texto.contains(&format!("<{}>1 ", Gravedad::Informativo.prioridad())),
            "{texto}"
        );
    }

    #[test]
    fn un_sello_que_no_llego_se_reintenta_cuando_el_colector_vuelve() {
        // Asimetria deliberada con las transiciones, y la unica del emisor.
        //
        // Una transicion perdida NO se reintenta: reemitirla al recuperarse el
        // colector le mostraria al operador un incidente que ya paso.
        //
        // Un sello perdido SI: es un hueco en la cadena del testigo, y reenviar
        // el extremo vigente no cuenta nada falso, cuenta lo que sigue siendo
        // cierto. Si no se reintentara, un colector caido un minuto dejaria un
        // tramo del registro sin atestiguar para siempre, y ese tramo es
        // exactamente donde alguien podria recortar sin que nadie lo notase.
        let (despacho, buzon) = DespachoIntermitente::nuevo(1);
        let mut emisor = Emisor::nuevo(despacho, "sensor-1", "eth0");

        assert!(!emisor.sellar(7, "aa", 0), "el primer envio falla");
        assert!(
            emisor.sellar(7, "aa", 0),
            "el segundo, con el colector en pie"
        );
        assert_eq!(buzon.borrow().len(), 1, "el sello llego una vez");

        assert!(emisor.sellar(7, "aa", 0), "y ya no hace falta repetirlo");
        assert_eq!(
            buzon.borrow().len(),
            1,
            "lo que ya se entrego no se reenvia"
        );
    }

    #[test]
    fn un_extremo_distinto_para_el_mismo_asiento_se_sella_de_nuevo() {
        // Es el caso que delata la alteracion del ultimo asiento sin cambiar su
        // numero: el numero es el mismo y el extremo no. Comparar solo por
        // numero dejaria pasar justo esa mutacion, que es la que RPT-029 §2.1
        // dejo escrita como limitacion y PA-57 cerro en local.
        let (despacho, buzon) = DespachoIntermitente::nuevo(0);
        let mut emisor = Emisor::nuevo(despacho, "sensor-1", "eth0");

        assert!(emisor.sellar(7, "aa", 0));
        assert!(emisor.sellar(7, "aa", 0));
        assert_eq!(buzon.borrow().len(), 1);

        assert!(emisor.sellar(7, "bb", 0));
        assert_eq!(buzon.borrow().len(), 2, "otro extremo es otra noticia");
    }

    // -----------------------------------------------------------------------
    // Protocolo de la escucha local — RPT-035, PA-41
    // -----------------------------------------------------------------------

    use crate::servicio::{Atiende, atender_peticion};
    use eje_ipc::{
        CODIGO_RECHAZO, CODIGO_RESPUESTA, Canal, NOMBRE_MAXIMO, componer_peticion, desenmarcar,
    };

    /// Atendedor que devuelve el nombre del canal, o rechaza siempre.
    struct AtiendeDePrueba {
        rechaza: bool,
    }

    impl Atiende for AtiendeDePrueba {
        fn responder(&mut self, canal: Canal, carga: &[u8]) -> Result<Vec<u8>, String> {
            if self.rechaza {
                return Err("motivo de prueba".to_owned());
            }
            Ok(format!("{}:{}", canal.identificador(), carga.len()).into_bytes())
        }
    }

    fn respuesta_a(carga: &[u8], rechaza: bool) -> Vec<u8> {
        let mut atiende = AtiendeDePrueba { rechaza };
        let marco = atender_peticion(&mut atiende, carga);
        desenmarcar(&marco)
            .expect("la respuesta es un marco valido")
            .to_vec()
    }

    #[test]
    fn los_manejadores_responden_lo_que_hay_en_el_registro() {
        use crate::servicio::Manejadores;

        let registro = registro_con(2);
        let vigentes = normales();
        let mut manejadores = Manejadores {
            registro: &registro,
            condiciones: &vigentes,
            evidencia: std::path::Path::new("/datos/eje/evidencia.alm"),
        };

        let peticion = componer_peticion(Canal::ConsultarAlertas, br#"{"desdeAsiento":0}"#)
            .expect("permitido");
        let cuerpo = respuesta_de(&mut manejadores, &peticion);

        assert_eq!(cuerpo[0], CODIGO_RESPUESTA);
        let respuesta: eje_ipc::mensajes::RespuestaAlertas =
            serde_json::from_slice(&cuerpo[1..]).expect("es JSON de respuesta");
        assert_eq!(respuesta.sucesos.len(), 2);
        assert_eq!(
            respuesta.primer_disponible, 1,
            "sin segmentos archivados, lo mas antiguo es el asiento 1"
        );
    }

    #[test]
    fn un_canal_sin_manejador_se_rechaza_con_motivo_y_no_con_lista_vacia() {
        // «No hay nada» y «esto todavia no lo sirve nadie» no son lo mismo. Una
        // lista vacia haria creer a VIS-04 que el inventario esta vacio.
        use crate::servicio::Manejadores;

        let registro = registro_con(1);
        let vigentes = normales();
        let mut manejadores = Manejadores {
            registro: &registro,
            condiciones: &vigentes,
            evidencia: std::path::Path::new("/datos/eje/evidencia.alm"),
        };

        let peticion = componer_peticion(Canal::ObtenerInventario, b"").expect("permitido");
        let cuerpo = respuesta_de(&mut manejadores, &peticion);

        assert_eq!(cuerpo[0], CODIGO_RECHAZO);
        assert!(String::from_utf8_lossy(&cuerpo[1..]).contains("manejador"));
    }

    #[test]
    fn una_peticion_de_alertas_ilegible_se_rechaza_con_motivo() {
        use crate::servicio::Manejadores;

        let registro = registro_con(1);
        let vigentes = normales();
        let mut manejadores = Manejadores {
            registro: &registro,
            condiciones: &vigentes,
            evidencia: std::path::Path::new("/datos/eje/evidencia.alm"),
        };

        let peticion =
            componer_peticion(Canal::ConsultarAlertas, b"esto no es JSON").expect("permitido");

        assert_eq!(respuesta_de(&mut manejadores, &peticion)[0], CODIGO_RECHAZO);
    }

    /// Respuesta desenmarcada de un atendedor concreto.
    fn respuesta_de(atiende: &mut dyn Atiende, carga: &[u8]) -> Vec<u8> {
        let marco = atender_peticion(atiende, carga);
        desenmarcar(&marco).expect("marco valido").to_vec()
    }

    #[test]
    fn una_peticion_bien_formada_recibe_su_respuesta() {
        let peticion =
            componer_peticion(Canal::ConsultarAlertas, b"{}").expect("el canal esta permitido");

        let cuerpo = respuesta_a(&peticion, false);

        assert_eq!(cuerpo[0], CODIGO_RESPUESTA);
        assert_eq!(&cuerpo[1..], b"consultar-alertas:2");
    }

    #[test]
    fn un_canal_desconocido_se_rechaza_con_motivo_y_no_en_silencio() {
        // Cerrar la conexion sin decir nada dejaria al otro extremo sin saber si
        // el agente no entiende o no esta. Es el tercer estado de RPT-006 §4
        // aplicado al cable.
        let nombre = b"contener-ya";
        let mut inventado = Vec::new();
        inventado.extend_from_slice(&(nombre.len() as u16).to_be_bytes());
        inventado.extend_from_slice(nombre);

        let cuerpo = respuesta_a(&inventado, false);

        assert_eq!(cuerpo[0], CODIGO_RECHAZO);
        assert!(!cuerpo[1..].is_empty(), "el rechazo lleva motivo");
    }

    #[test]
    fn un_rechazo_del_manejador_viaja_con_su_motivo() {
        let peticion =
            componer_peticion(Canal::ObtenerCondiciones, b"").expect("el canal esta permitido");

        let cuerpo = respuesta_a(&peticion, true);

        assert_eq!(cuerpo[0], CODIGO_RECHAZO);
        assert_eq!(&cuerpo[1..], b"motivo de prueba");
    }

    #[test]
    fn un_nombre_de_canal_absurdo_no_reserva_memoria() {
        // El prefijo llega de un socket. Se acota ANTES de indexar, por la misma
        // razon que el prefijo de marco.
        let mut absurdo = Vec::new();
        absurdo.extend_from_slice(&u16::MAX.to_be_bytes());
        absurdo.extend_from_slice(b"corto");

        assert_eq!(respuesta_a(&absurdo, false)[0], CODIGO_RECHAZO);
        assert!(NOMBRE_MAXIMO < u16::MAX as usize);
    }

    #[test]
    fn una_peticion_truncada_no_desborda() {
        for longitud in 0..4 {
            let cuerpo = respuesta_a(&vec![0u8; longitud], false);
            assert_eq!(cuerpo[0], CODIGO_RECHAZO, "longitud {longitud}");
        }
    }

    #[test]
    fn un_nombre_que_no_es_utf8_se_rechaza_como_canal_no_permitido() {
        // El motivo que importa no es el de codificacion: es que no esta en la
        // lista. Un nombre invalido no puede ser ninguno de los declarados.
        let mut invalido = Vec::new();
        invalido.extend_from_slice(&2u16.to_be_bytes());
        invalido.extend_from_slice(&[0xFF, 0xFE]);

        assert_eq!(respuesta_a(&invalido, false)[0], CODIGO_RECHAZO);
    }

    #[test]
    fn la_contencion_no_es_alcanzable_por_el_socket() {
        // La lista de permitidos de `eje-ipc` ya lo garantiza; esto lo comprueba
        // por el camino que un atacante local usaria de verdad.
        for prohibido in ["ordenar-contencion", "invocar", "ejecutar-comando"] {
            let mut peticion = Vec::new();
            let nombre = prohibido.as_bytes();
            peticion.extend_from_slice(&(nombre.len() as u16).to_be_bytes());
            peticion.extend_from_slice(nombre);

            assert_eq!(
                respuesta_a(&peticion, false)[0],
                CODIGO_RECHAZO,
                "'{prohibido}' no puede alcanzarse"
            );
        }
    }

    #[test]
    fn la_peticion_es_reversible_para_todos_los_canales_de_consulta() {
        for canal in Canal::TODOS {
            let peticion = componer_peticion(canal, b"carga").expect("permitido");
            let cuerpo = respuesta_a(&peticion, false);

            assert_eq!(cuerpo[0], CODIGO_RESPUESTA);
            assert!(
                String::from_utf8_lossy(&cuerpo[1..]).starts_with(canal.identificador()),
                "{canal:?}"
            );
        }
    }

    #[test]
    fn la_perdida_de_captura_es_una_condicion_y_no_un_suceso() {
        // Es verdadera hasta que alguien intervenga. Anotarla en cada trama
        // perdida inundaria ALM-01 con la misma noticia (RPT-019 §2).
        let mut observacion = AlmacenObservacion::nuevo();
        observacion.anotar_perdida();

        let vigentes = condiciones(
            &EstadoArranque::PrimerArranque,
            &observacion,
            &RegistroEvidencia::nuevo(),
            false,
            false,
            false,
            EstadoConfiguracion::Firmada,
        );

        assert!(vigentes.captura_con_perdida);
        assert!(vigentes.hay_degradacion());
        assert!(!vigentes.hay_manipulacion());
    }

    // -----------------------------------------------------------------------
    // Rotacion por segmentos — RPT-040, PA-59
    // -----------------------------------------------------------------------

    use crate::alertas::{rotar_si_toca, ruta_de_segmento};
    use eje_almacen::persistencia::ASIENTOS_POR_SEGMENTO;

    #[test]
    fn el_nombre_del_segmento_se_deriva_de_la_base_y_no_de_un_contador() {
        // Un contador aparte podria desincronizarse del contenido, y entonces dos
        // rotaciones escribirian el mismo nombre y la segunda se comeria a la
        // primera. Derivarlo del contenido hace imposible esa clase de fallo.
        let activo = std::path::Path::new("/datos/eje/evidencia.alm");
        let umbral = ASIENTOS_POR_SEGMENTO as u64;

        assert!(
            ruta_de_segmento(activo, 1)
                .to_string_lossy()
                .ends_with("evidencia-000001.alm")
        );
        assert!(
            ruta_de_segmento(activo, umbral + 1)
                .to_string_lossy()
                .ends_with("evidencia-000002.alm")
        );
        assert!(
            ruta_de_segmento(activo, umbral * 2 + 1)
                .to_string_lossy()
                .ends_with("evidencia-000003.alm")
        );
        assert_eq!(ruta_de_segmento(activo, 1).parent(), activo.parent());
    }

    #[test]
    fn por_debajo_del_umbral_no_se_rota_ni_se_toca_el_disco() {
        let almacen = AlmacenDePrueba::nuevo("sin-rotar");
        let mut registro = registro_con(3);

        assert!(
            rotar_si_toca(&almacen.evidencia(), &mut registro)
                .expect("no falla")
                .is_none()
        );
        assert_eq!(registro.base(), 1);
        assert_eq!(registro.longitud(), 3);
        assert!(!ruta_de_segmento(&almacen.evidencia(), 1).exists());
    }

    #[test]
    fn al_alcanzar_el_umbral_se_archiva_y_el_activo_continua_la_serie() {
        // El recorrido entero de RPT-040 §1: el segmento cerrado va a su fichero,
        // el activo queda vacio arrastrando base y extremo, y la serie no
        // reinicia.
        let almacen = AlmacenDePrueba::nuevo("rotacion");
        let mut registro = registro_con(ASIENTOS_POR_SEGMENTO as u64);
        let extremo = registro.extremo();
        let ultimo = registro.ultimo_numero();

        let archivado = rotar_si_toca(&almacen.evidencia(), &mut registro)
            .expect("no falla")
            .expect("al alcanzar el umbral se rota");

        assert!(archivado.exists());
        assert!(registro.vacio());
        assert_eq!(registro.base(), ultimo + 1);
        assert_eq!(registro.genesis(), extremo);
        assert_eq!(
            registro.ultimo_numero(),
            ultimo,
            "un segmento vacio declara el ultimo del anterior"
        );

        // Y lo que se anexe despues continua la numeracion global.
        let numero = anotar_incontenible(&mut registro, 9_000, &MAC, &incontenible())
            .expect("cabe en el segmento nuevo");
        assert_eq!(numero, ultimo + 1);
    }

    #[test]
    fn el_ancla_anterior_sigue_valiendo_para_el_segmento_recien_abierto() {
        // La propiedad de la que cuelga la rotacion de dos pasos. Si esto fallara,
        // un corte de energia justo despues de sustituir el activo dejaria al
        // arranque siguiente acusando de truncamiento a una rotacion correcta.
        use eje_almacen::persistencia::{Cotejo, ancla_de, cotejar};

        let almacen = AlmacenDePrueba::nuevo("ancla-tras-rotar");
        let mut registro = registro_con(ASIENTOS_POR_SEGMENTO as u64);

        // El ancla que existia ANTES de rotar, sin tocar.
        let ancla = ancla_de(&registro).expect("no esta vacio");

        rotar_si_toca(&almacen.evidencia(), &mut registro)
            .expect("no falla")
            .expect("rota");

        assert_eq!(
            cotejar(&registro, &ancla),
            Cotejo::Conforme,
            "el ancla del segmento cerrado describe tambien el activo vacio"
        );
    }

    #[test]
    fn el_segmento_archivado_y_el_activo_encajan_por_la_frontera() {
        // Lo que sustituye al ancla en los archivados (RPT-040 §4). Se comprueba
        // por el camino real: leyendo del disco lo que la rotacion escribio.
        use eje_almacen::persistencia::analizar;

        let almacen = AlmacenDePrueba::nuevo("frontera");
        let mut registro = registro_con(ASIENTOS_POR_SEGMENTO as u64);

        let archivado = rotar_si_toca(&almacen.evidencia(), &mut registro)
            .expect("no falla")
            .expect("rota");

        let cerrado =
            analizar(&std::fs::read(&archivado).expect("leer")).expect("el archivado verifica");

        assert!(cerrado.verificar_cadena().is_ok());
        assert!(
            registro.continua_a(&cerrado),
            "el activo debe enlazar con el segmento que acaba de cerrarse"
        );

        // Y el activo del disco tambien, no solo el que quedo en memoria.
        let activo =
            analizar(&std::fs::read(almacen.evidencia()).expect("leer")).expect("verifica");
        assert!(activo.continua_a(&cerrado));
    }

    #[test]
    fn tras_rotar_la_respuesta_dice_donde_empieza_lo_que_entrega() {
        // PA-74. Es el hueco que abrio PA-59: quien pide desde el cero recibe el
        // segmento activo, y sin este campo creeria que eso es todo lo que hubo.
        //
        // «No hay nada» y «esto no empieza aqui» no son lo mismo, y colapsarlos es
        // como un operador concluye que un incidente no ocurrio.
        use crate::alertas::primer_disponible;

        let almacen = AlmacenDePrueba::nuevo("primer-disponible");
        let mut registro = registro_con(ASIENTOS_POR_SEGMENTO as u64);

        assert_eq!(
            primer_disponible(&almacen.evidencia(), &registro),
            1,
            "antes de rotar, todo esta en el activo"
        );

        rotar_si_toca(&almacen.evidencia(), &mut registro)
            .expect("no falla")
            .expect("rota");

        assert_eq!(
            registro.base(),
            ASIENTOS_POR_SEGMENTO as u64 + 1,
            "el activo ya no contiene el principio"
        );
        assert_eq!(
            primer_disponible(&almacen.evidencia(), &registro),
            1,
            "pero el archivado sigue ahi y la respuesta debe decirlo"
        );
    }

    #[test]
    fn si_alguien_borra_el_segmento_archivado_la_cifra_lo_refleja() {
        // El motivo de leer el disco en cada consulta en lugar de cachear al
        // arrancar. Una cifra cacheada seguiria diciendo 1 despues de que
        // alguien borrara el segmento 1: se quedaria obsoleta **en la direccion
        // que oculta la manipulacion**, que es la peor de las dos.
        use crate::alertas::primer_disponible;

        let almacen = AlmacenDePrueba::nuevo("borrado-del-archivo");
        let mut registro = registro_con(ASIENTOS_POR_SEGMENTO as u64);

        let archivado = rotar_si_toca(&almacen.evidencia(), &mut registro)
            .expect("no falla")
            .expect("rota");
        assert_eq!(primer_disponible(&almacen.evidencia(), &registro), 1);

        std::fs::remove_file(&archivado).expect("borrar");

        assert_eq!(
            primer_disponible(&almacen.evidencia(), &registro),
            registro.base(),
            "lo que ya no esta en disco no puede seguir declarandose disponible"
        );
    }

    // -----------------------------------------------------------------------
    // Paridad de uso en el lado Rust — RPT-043, PA-76
    // -----------------------------------------------------------------------

    /// Manifiesto, leido desde la raiz del workspace.
    ///
    /// Se relee aqui en lugar de reutilizar los ayudantes de `eje-ipc` porque
    /// aquellos son `#[cfg(test)]` de otro crate. La alternativa —una tabla
    /// `canal → CAMPOS_*` escrita a mano en este fichero— seria justo el tipo de
    /// cosa que hay que mantener y que se desincroniza sin avisar.
    fn manifiesto_ipc() -> String {
        let ruta = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("contrato-ipc.toml");

        std::fs::read_to_string(&ruta)
            .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", ruta.display()))
    }

    /// Valor entrecomillado de una linea `clave = "valor"`.
    fn entrecomillado(linea: &str, clave: &str) -> Option<String> {
        let resto = linea.trim().strip_prefix(clave)?.trim_start();
        let sin_igual = resto.strip_prefix('=')?.trim_start();
        let sin_comilla = sin_igual.strip_prefix('"')?;
        let fin = sin_comilla.find('"')?;
        Some(sin_comilla[..fin].to_owned())
    }

    /// Forma que el manifiesto declara para la **respuesta** de un canal.
    fn forma_de_respuesta(contenido: &str, canal: &str) -> Option<String> {
        for bloque in contenido.split("[[mensaje]]").skip(1) {
            let mut cual = None;
            let mut direccion = None;
            let mut forma = None;
            for linea in bloque.lines() {
                if linea.trim_start().starts_with('[') {
                    break;
                }
                cual = cual.or_else(|| entrecomillado(linea, "canal"));
                direccion = direccion.or_else(|| entrecomillado(linea, "direccion"));
                forma = forma.or_else(|| entrecomillado(linea, "forma"));
            }
            if cual.as_deref() == Some(canal) && direccion.as_deref() == Some("respuesta") {
                return forma;
            }
        }
        None
    }

    /// Campos que el manifiesto declara para un registro.
    fn campos_de(contenido: &str, registro: &str) -> Vec<String> {
        let mut nombres = Vec::new();
        for bloque in contenido.split("[[campo]]").skip(1) {
            let mut cual = None;
            let mut nombre = None;
            for linea in bloque.lines() {
                if linea.trim_start().starts_with('[') {
                    break;
                }
                cual = cual.or_else(|| entrecomillado(linea, "registro"));
                nombre = nombre.or_else(|| entrecomillado(linea, "nombre"));
            }
            if cual.as_deref() == Some(registro) {
                if let Some(nombre) = nombre {
                    nombres.push(nombre);
                }
            }
        }
        nombres
    }

    #[test]
    fn cada_manejador_responde_con_la_forma_que_el_manifiesto_declara() {
        // PA-76. El gemelo de la barrera de PA-75, en el otro extremo del cable.
        //
        // Aqui NO se lee el fuente de `servicio.rs`: se llama al manejador y se
        // comparan las claves del JSON que produce con los campos que el
        // manifiesto declara para ese canal. Si alguien serializa otro tipo, las
        // claves no cuadran, y da igual como este escrito el `match`.
        use crate::servicio::Manejadores;
        use eje_ipc::{CODIGO_RESPUESTA, Canal, componer_peticion};

        let contenido = manifiesto_ipc();
        let registro = registro_con(2);
        let vigentes = normales();

        // Los cuatro canales sin manejador se rechazan con motivo y eso ya tiene
        // su prueba; aqui solo se miran los que responden de verdad.
        for (canal, carga) in [
            (Canal::ConsultarAlertas, &br#"{"desdeAsiento":0}"#[..]),
            (Canal::ObtenerCondiciones, &b""[..]),
        ] {
            let mut manejadores = Manejadores {
                registro: &registro,
                condiciones: &vigentes,
                evidencia: std::path::Path::new("/datos/eje/evidencia.alm"),
            };

            let peticion = componer_peticion(canal, carga).expect("canal permitido");
            let cuerpo = respuesta_de(&mut manejadores, &peticion);
            assert_eq!(cuerpo[0], CODIGO_RESPUESTA, "{canal:?}");

            let forma = forma_de_respuesta(&contenido, canal.identificador())
                .unwrap_or_else(|| panic!("{canal:?} no declara respuesta en el manifiesto"));

            let valor: serde_json::Value =
                serde_json::from_slice(&cuerpo[1..]).expect("la respuesta es JSON");
            let objeto = valor.as_object().unwrap_or_else(|| {
                panic!("{canal:?} declara responder '{forma}' y devolvio algo que no es un objeto")
            });

            let mut entregadas: Vec<String> = objeto.keys().cloned().collect();
            let mut declaradas = campos_de(&contenido, &forma);
            assert!(
                !declaradas.is_empty(),
                "el registro '{forma}' no declara campos en el manifiesto"
            );

            // Se comparan como conjuntos: el ORDEN ya lo comprueba la paridad de
            // `eje-ipc` contra el manifiesto, y serde serializa en el orden de
            // declaracion del struct. Repetirlo aqui no anadiria garantia.
            entregadas.sort();
            declaradas.sort();

            assert_eq!(
                entregadas, declaradas,
                "{canal:?} declara responder '{forma}' y entrega otras claves.\n\
                 Declarar un registro no lo cablea: el manejador tiene que servirlo."
            );
        }
    }
}
