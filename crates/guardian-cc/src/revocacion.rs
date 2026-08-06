//! Revocacion de la clave de inventario.
//!
//! RPT-015, PA-33.
//!
//! # Que resuelve
//!
//! La autoridad del inventario descansa en una sola clave. Si se filtra, el
//! atacante emite secuencias crecientes igual que el legitimo: la monotonia de
//! RPT-012 no le estorba.
//!
//! # Tres decisiones que se hacen mal por defecto
//!
//! ## La revocacion no es total
//!
//! Invalidar «todo lo firmado por K» invalida tambien los inventarios legitimos
//! anteriores al compromiso. El agente se quedaria sin marcados, y sin marcados
//! los equipos criticos dejan de estar protegidos: seria provocarnos la perdida
//! que el atacante buscaba.
//!
//! Por eso el certificado lleva una **secuencia de corte**: cae lo firmado por
//! encima de ella, sobrevive lo de por debajo.
//!
//! ## Quien firma no puede ser ninguna de las dos claves conocidas
//!
//! Ni la operativa —el atacante la tiene— ni la de PremosCorp, porque
//! [`DominioClave`] existe desde RPT-011 para que el proveedor no pueda decidir
//! que equipos del cliente son criticos. Hace falta un tercer dominio,
//! [`DominioClave::ClienteRecuperacion`], en custodia del cliente y fuera de
//! linea.
//!
//! ## El certificado **baja** el centinela
//!
//! Es la enmienda de RPT-015 §6.1 a la regla «el centinela nunca retrocede».
//!
//! Sin ella, un atacante con la clave operativa emite un inventario con secuencia
//! `u64::MAX`, el agente lo acepta —la firma es valida— y ningun inventario
//! legitimo puede ya superarlo. El inventario queda congelado **para siempre**,
//! con un solo mensaje, y revocar no lo arregla porque el centinela sigue arriba.
//!
//! El resultado seria peor que el compromiso. De ahi que un certificado valido
//! reinicie el centinela a su secuencia de corte: es la unica operacion
//! autorizada a bajar la marca de agua, y es segura porque exige la clave que el
//! atacante no tiene.

use eje_almacen::resumen::{Absorbedor, Resumen};
use motor_pqc::firma_hibrida::{ClaveVerificacionHibrida, FirmaHibrida, verificar};

use crate::inventario::{ClaveInventario, DominioClave};

/// Dominio del resumen que identifica una clave.
const DOMINIO_IDENTIFICADOR: &[u8] = b"eje-latam/agt-01/identificador-clave/v1";

/// Dominio del mensaje firmado de un certificado de revocacion.
///
/// Separado del de la raiz del inventario: sin etiquetas distintas, una firma
/// sobre un certificado podria presentarse como firma de raiz.
const DOMINIO_CERTIFICADO: &[u8] = b"eje-latam/agt-01/certificado-revocacion/v1";

/// Identificador estable de una clave de verificacion.
///
/// Es el resumen de su forma serializada. Se usa en lugar de la clave completa
/// porque el registro de revocaciones debe poder nombrar una clave que ya no se
/// conserva.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdentificadorClave(Resumen);

impl IdentificadorClave {
    /// Deriva el identificador de una clave de verificacion.
    #[must_use]
    pub fn de(clave: &ClaveVerificacionHibrida) -> Self {
        let mut absorbedor = Absorbedor::nuevo(DOMINIO_IDENTIFICADOR);
        absorbedor.campo(&clave.a_bytes());
        Self(absorbedor.finalizar())
    }

    /// Resumen subyacente, para registro forense.
    #[must_use]
    pub const fn resumen(&self) -> &Resumen {
        &self.0
    }
}

/// Errores de la revocacion.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorRevocacion {
    /// El certificado no lo firma una clave del dominio de recuperacion.
    ///
    /// Admitir aqui la clave operativa dejaria que el atacante se
    /// «autorrevocase» a una secuencia de corte alta, que es lo contrario de una
    /// revocacion.
    #[error("el certificado exige una clave de recuperacion; se presento {encontrado:?}")]
    DominioDeClaveIncorrecto {
        /// Dominio de la clave presentada.
        encontrado: DominioClave,
    },

    /// La firma del certificado no verifica.
    #[error("la firma del certificado de revocacion no verifica")]
    FirmaInvalida,

    /// El certificado se revoca a si mismo.
    ///
    /// Revocar la clave sucesora en el mismo acto dejaria al cliente sin
    /// autoridad ninguna sobre su inventario.
    #[error("el certificado declara la misma clave como revocada y como sucesora")]
    SucesoraEsLaRevocada,
}

/// Certificado tal como llega, **sin verificar**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertificadoRevocacion {
    /// Clave que deja de valer por encima del corte.
    pub revocada: IdentificadorClave,
    /// Secuencia de corte. Lo firmado por `revocada` **por encima** de este
    /// valor deja de aceptarse; lo de por debajo sigue siendo valido.
    pub hasta_secuencia: u64,
    /// Clave que sustituye a la revocada.
    pub sucesora: IdentificadorClave,
    /// Instante de emision, en segundos desde la epoca.
    pub emitido_en: u64,
}

/// Mensaje canonico que la clave de recuperacion firma.
#[must_use]
pub fn mensaje_de_certificado(certificado: &CertificadoRevocacion) -> Vec<u8> {
    let mut absorbedor = Absorbedor::nuevo(DOMINIO_CERTIFICADO);
    absorbedor
        .resumen(certificado.revocada.resumen())
        .entero(certificado.hasta_secuencia)
        .resumen(certificado.sucesora.resumen())
        .entero(certificado.emitido_en);
    absorbedor.finalizar().bytes().to_vec()
}

/// Certificado cuya firma y dominio de clave ya se comprobaron.
///
/// Campos privados y sin constructor publico salvo [`Self::verificar`]. Que
/// exista **es** la prueba de que lo firmo la clave de recuperacion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertificadoVerificado {
    revocada: IdentificadorClave,
    hasta_secuencia: u64,
    sucesora: IdentificadorClave,
}

impl CertificadoVerificado {
    /// Comprueba dominio de clave, coherencia y firma.
    ///
    /// # Errores
    ///
    /// [`ErrorRevocacion::DominioDeClaveIncorrecto`],
    /// [`ErrorRevocacion::SucesoraEsLaRevocada`] o
    /// [`ErrorRevocacion::FirmaInvalida`].
    pub fn verificar(
        certificado: CertificadoRevocacion,
        firma: &FirmaHibrida,
        clave: &ClaveInventario,
    ) -> Result<Self, ErrorRevocacion> {
        if clave.dominio() != DominioClave::ClienteRecuperacion {
            return Err(ErrorRevocacion::DominioDeClaveIncorrecto {
                encontrado: clave.dominio(),
            });
        }

        if certificado.revocada == certificado.sucesora {
            return Err(ErrorRevocacion::SucesoraEsLaRevocada);
        }

        verificar(
            clave.verificacion(),
            &mensaje_de_certificado(&certificado),
            firma,
        )
        .map_err(|_| ErrorRevocacion::FirmaInvalida)?;

        Ok(Self {
            revocada: certificado.revocada,
            hasta_secuencia: certificado.hasta_secuencia,
            sucesora: certificado.sucesora,
        })
    }

    /// Clave revocada.
    #[must_use]
    pub const fn revocada(&self) -> IdentificadorClave {
        self.revocada
    }

    /// Secuencia de corte.
    #[must_use]
    pub const fn hasta_secuencia(&self) -> u64 {
        self.hasta_secuencia
    }

    /// Clave sucesora.
    #[must_use]
    pub const fn sucesora(&self) -> IdentificadorClave {
        self.sucesora
    }
}

/// Registro local de claves revocadas.
///
/// # Por que basta un fichero y no hace falta el ancla de PA-28
///
/// A diferencia del centinela de frescura, este conjunto **solo crece** y el
/// certificado se puede **volver a presentar**. Perderlo devuelve el sistema al
/// estado de antes de la revocacion, que es el estado en el que ya vivimos; no
/// por debajo. El cliente conserva el certificado y reponerlo es presentarlo de
/// nuevo (RPT-015 §5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegistroRevocaciones {
    entradas: Vec<(IdentificadorClave, u64)>,
}

impl RegistroRevocaciones {
    /// Registro vacio.
    #[must_use]
    pub const fn nuevo() -> Self {
        Self {
            entradas: Vec::new(),
        }
    }

    /// Anota un certificado verificado.
    ///
    /// Si la clave ya figuraba, **se conserva el corte mas bajo**. Un corte
    /// posterior mas alto aflojaria una revocacion existente, y una revocacion
    /// que se puede aflojar no es una revocacion.
    pub fn anotar(&mut self, certificado: &CertificadoVerificado) {
        let revocada = certificado.revocada();
        let corte = certificado.hasta_secuencia();

        if let Some(entrada) = self
            .entradas
            .iter_mut()
            .find(|(identificador, _)| *identificador == revocada)
        {
            entrada.1 = entrada.1.min(corte);
            return;
        }

        self.entradas.push((revocada, corte));
    }

    /// Corte anotado para una clave, si esta revocada.
    #[must_use]
    pub fn corte_de(&self, identificador: &IdentificadorClave) -> Option<u64> {
        self.entradas
            .iter()
            .find(|(anotado, _)| anotado == identificador)
            .map(|(_, corte)| *corte)
    }

    /// Indica si la clave puede haber firmado esa secuencia.
    ///
    /// Una clave no revocada puede firmar cualquiera. Una revocada, solo hasta su
    /// corte inclusive.
    #[must_use]
    pub fn admite(&self, identificador: &IdentificadorClave, secuencia: u64) -> bool {
        match self.corte_de(identificador) {
            None => true,
            Some(corte) => secuencia <= corte,
        }
    }

    /// Numero de claves revocadas.
    #[must_use]
    pub fn anotadas(&self) -> usize {
        self.entradas.len()
    }
}

// ---------------------------------------------------------------------------
// Persistencia — RPT-016, PA-34
// ---------------------------------------------------------------------------

/// Numero magico del fichero de revocaciones.
pub const MAGICO_REVOCACIONES: &[u8; 8] = b"EJE-REV1";

/// Version del formato.
pub const VERSION_REVOCACIONES: u16 = 1;

/// Cabecera: magico, version y numero de anotaciones.
const CABECERA_REVOCACIONES: usize = 8 + 2 + 4;

/// Parte fija de una anotacion, sin contar la firma.
const CUERPO_ANOTACION: usize = 32 + 8 + 32 + 8;

/// Cota de anotaciones. Una revocacion es un evento raro; mil es holgado.
pub const ANOTACIONES_MAXIMAS: usize = 1_000;

/// Certificado junto a la firma que lo respalda.
///
/// Se conserva la firma **a proposito**: el fichero de revocaciones vive en un
/// almacen que el modelo de amenazas asume manipulable, y guardar solo el par
/// derivado (identificador, corte) permitiria subir un corte y aflojar una
/// revocacion en silencio. Con la firma, la manipulacion se detecta al cargar.
#[derive(Clone)]
pub struct Anotacion {
    /// Certificado tal como se emitio.
    pub certificado: CertificadoRevocacion,
    /// Firma de la clave de recuperacion.
    pub firma: FirmaHibrida,
}

/// Defectos del fichero de revocaciones.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorArchivo {
    /// El fichero no empieza por [`MAGICO_REVOCACIONES`].
    #[error("el fichero no es un registro de revocaciones de Eje-Latam")]
    MagicoAusente,

    /// Version desconocida.
    #[error("version de formato {encontrada}; este binario entiende la {VERSION_REVOCACIONES}")]
    VersionDesconocida {
        /// Version leida.
        encontrada: u16,
    },

    /// El fichero termina antes de lo que su estructura exige.
    #[error("fichero truncado: se esperaban {esperados} bytes y hay {disponibles}")]
    Truncado {
        /// Bytes que la estructura exige.
        esperados: usize,
        /// Bytes disponibles.
        disponibles: usize,
    },

    /// Quedaron bytes sin interpretar.
    #[error("{sobrantes} bytes sobrantes al final del fichero")]
    BytesSobrantes {
        /// Bytes no interpretados.
        sobrantes: usize,
    },

    /// Se declaran mas anotaciones de las admitidas.
    #[error("se declaran {declaradas} anotaciones; el maximo es {ANOTACIONES_MAXIMAS}")]
    DemasiadasAnotaciones {
        /// Numero declarado.
        declaradas: usize,
    },

    /// Las anotaciones no vienen en orden ascendente de clave revocada.
    #[error("la anotacion {posicion} rompe el orden ascendente")]
    Desordenadas {
        /// Indice de la primera fuera de orden.
        posicion: usize,
    },

    /// Una firma no decodifica.
    #[error("la firma de la anotacion {posicion} no decodifica")]
    FirmaMalformada {
        /// Indice de la anotacion.
        posicion: usize,
    },

    /// Una anotacion no supera la verificacion.
    ///
    /// Es el sintoma de manipulacion: alguien edito el fichero para aflojar o
    /// inventar una revocacion.
    #[error("la anotacion {posicion} no verifica: {detalle}")]
    NoVerifica {
        /// Indice de la anotacion.
        posicion: usize,
        /// Motivo.
        detalle: String,
    },
}

/// Registro persistible: certificados con sus firmas, en orden canonico.
#[derive(Clone, Default)]
pub struct ArchivoRevocaciones {
    anotaciones: Vec<Anotacion>,
}

impl ArchivoRevocaciones {
    /// Archivo vacio.
    #[must_use]
    pub const fn nuevo() -> Self {
        Self {
            anotaciones: Vec::new(),
        }
    }

    /// Incorpora una anotacion ya verificada.
    ///
    /// Si la clave ya figuraba, se conserva la del **corte mas bajo**, igual que
    /// en [`RegistroRevocaciones::anotar`]: una revocacion que se puede aflojar
    /// no es una revocacion.
    pub fn anotar(&mut self, anotacion: Anotacion) {
        let revocada = anotacion.certificado.revocada;

        if let Some(existente) = self
            .anotaciones
            .iter_mut()
            .find(|previa| previa.certificado.revocada == revocada)
        {
            if anotacion.certificado.hasta_secuencia < existente.certificado.hasta_secuencia {
                *existente = anotacion;
            }
            return;
        }

        self.anotaciones.push(anotacion);
        self.anotaciones
            .sort_unstable_by_key(|anotacion| anotacion.certificado.revocada);
    }

    /// Anotaciones en orden canonico.
    #[must_use]
    pub fn anotaciones(&self) -> &[Anotacion] {
        &self.anotaciones
    }

    /// Registro derivado, para el sexto eslabon.
    #[must_use]
    pub fn registro(&self) -> RegistroRevocaciones {
        let mut registro = RegistroRevocaciones::nuevo();
        for anotacion in &self.anotaciones {
            registro.entradas.push((
                anotacion.certificado.revocada,
                anotacion.certificado.hasta_secuencia,
            ));
        }
        registro
    }

    /// Serializa al formato en disco.
    ///
    /// Mismas reglas que el inventario: cabecera con magico y version,
    /// anotaciones de ancho fijo para poder validar el numero declarado contra
    /// los bytes restantes antes de reservar, y orden canonico.
    #[must_use]
    pub fn serializar(&self) -> Vec<u8> {
        let longitud_firma = FirmaHibrida::longitud_serializada();
        let mut salida = Vec::with_capacity(
            CABECERA_REVOCACIONES + self.anotaciones.len() * (CUERPO_ANOTACION + longitud_firma),
        );

        salida.extend_from_slice(MAGICO_REVOCACIONES);
        salida.extend_from_slice(&VERSION_REVOCACIONES.to_be_bytes());
        salida.extend_from_slice(&(self.anotaciones.len() as u32).to_be_bytes());

        for anotacion in &self.anotaciones {
            let certificado = &anotacion.certificado;
            salida.extend_from_slice(certificado.revocada.resumen().bytes());
            salida.extend_from_slice(&certificado.hasta_secuencia.to_be_bytes());
            salida.extend_from_slice(certificado.sucesora.resumen().bytes());
            salida.extend_from_slice(&certificado.emitido_en.to_be_bytes());
            salida.extend_from_slice(&anotacion.firma.a_bytes());
        }

        salida
    }

    /// Analiza el fichero **y reverifica cada anotacion**.
    ///
    /// # Por que se reverifica al cargar
    ///
    /// El fichero vive donde el atacante escribe. Sin reverificar, editar un
    /// corte al alza aflojaria una revocacion sin dejar rastro. Con la firma
    /// presente, la unica manipulacion que queda es **borrar** el fichero, y eso
    /// devuelve al estado previo a la revocacion, que es recuperable
    /// volviendo a presentar el certificado (RPT-015 §5).
    ///
    /// # Errores
    ///
    /// Una variante de [`ErrorArchivo`] por defecto. Se distinguen los
    /// estructurales de los de verificacion: los primeros son un fichero roto y
    /// los segundos, manipulacion.
    pub fn analizar(bytes: &[u8], clave: &ClaveInventario) -> Result<Self, ErrorArchivo> {
        if bytes.len() < CABECERA_REVOCACIONES {
            return Err(ErrorArchivo::Truncado {
                esperados: CABECERA_REVOCACIONES,
                disponibles: bytes.len(),
            });
        }

        if &bytes[..8] != MAGICO_REVOCACIONES {
            return Err(ErrorArchivo::MagicoAusente);
        }

        let version = u16::from_be_bytes([bytes[8], bytes[9]]);
        if version != VERSION_REVOCACIONES {
            return Err(ErrorArchivo::VersionDesconocida {
                encontrada: version,
            });
        }

        let mut brutas = [0u8; 4];
        brutas.copy_from_slice(&bytes[10..14]);
        let declaradas = u32::from_be_bytes(brutas) as usize;

        if declaradas > ANOTACIONES_MAXIMAS {
            return Err(ErrorArchivo::DemasiadasAnotaciones { declaradas });
        }

        let longitud_firma = FirmaHibrida::longitud_serializada();
        let ancho = CUERPO_ANOTACION + longitud_firma;
        let esperados = CABECERA_REVOCACIONES + declaradas * ancho;

        if bytes.len() < esperados {
            return Err(ErrorArchivo::Truncado {
                esperados,
                disponibles: bytes.len(),
            });
        }
        if bytes.len() > esperados {
            return Err(ErrorArchivo::BytesSobrantes {
                sobrantes: bytes.len() - esperados,
            });
        }

        let mut archivo = Self::nuevo();
        let mut anterior: Option<IdentificadorClave> = None;
        let mut desplazamiento = CABECERA_REVOCACIONES;

        for posicion in 0..declaradas {
            let bloque = &bytes[desplazamiento..desplazamiento + ancho];

            let mut revocada = [0u8; 32];
            revocada.copy_from_slice(&bloque[..32]);
            let revocada = IdentificadorClave(Resumen::desde_bytes(revocada));

            if let Some(previa) = anterior {
                if revocada <= previa {
                    return Err(ErrorArchivo::Desordenadas { posicion });
                }
            }
            anterior = Some(revocada);

            let mut corte = [0u8; 8];
            corte.copy_from_slice(&bloque[32..40]);

            let mut sucesora = [0u8; 32];
            sucesora.copy_from_slice(&bloque[40..72]);

            let mut emision = [0u8; 8];
            emision.copy_from_slice(&bloque[72..80]);

            let certificado = CertificadoRevocacion {
                revocada,
                hasta_secuencia: u64::from_be_bytes(corte),
                sucesora: IdentificadorClave(Resumen::desde_bytes(sucesora)),
                emitido_en: u64::from_be_bytes(emision),
            };

            let firma = FirmaHibrida::desde_bytes(&bloque[CUERPO_ANOTACION..])
                .map_err(|_| ErrorArchivo::FirmaMalformada { posicion })?;

            CertificadoVerificado::verificar(certificado, &firma, clave).map_err(|error| {
                ErrorArchivo::NoVerifica {
                    posicion,
                    detalle: error.to_string(),
                }
            })?;

            archivo.anotaciones.push(Anotacion { certificado, firma });
            desplazamiento += ancho;
        }

        Ok(archivo)
    }
}
