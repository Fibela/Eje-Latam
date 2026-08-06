//! Adaptador de almacen local: del fichero al `ProveedorInventario`.
//!
//! RPT-013, PA-24.
//!
//! # Donde encaja
//!
//! ```text
//!   fichero .inv          analizar()            RaizVerificada::verificar()
//!   en disco     ───────► FicheroInventario ──► (eslabones 3, 4 y 5)
//!                            │                          │
//!                            │  bytes sobrantes,        │  firma, dominio,
//!                            │  truncado, magico...     │  frescura
//!                            ▼                          ▼
//!                       ErrorFormato            InventarioLocal
//!                                                       │
//!                                          marcado(mac) │ eslabones 1 y 2
//!                                                       ▼
//!                                              MarcadoVerificado
//! ```
//!
//! # La verificacion ocurre al cargar, no al consultar
//!
//! `InventarioLocal::cargar` cierra los cinco eslabones **una vez**. Cada
//! consulta posterior solo construye la prueba de inclusion y comprueba los dos
//! que dependen del marcado concreto.
//!
//! La alternativa —verificar la firma en cada consulta— seria mas lenta y, peor,
//! invitaria a saltarsela «por rendimiento» en algun camino. Un
//! [`InventarioLocal`] que existe es un inventario que ya paso por todo.

use eje_almacen::merkle::prueba_inclusion;

use crate::formato::{ErrorFormato, analizar};
use crate::inventario::{
    Centinela, ClaveInventario, ErrorInventario, Inventario, MarcadoVerificado, RaizVerificada,
};
use crate::proveedores::{DireccionEnlace, ErrorProveedor, ProveedorInventario};
use crate::revocacion::RegistroRevocaciones;

/// Fallo al cargar el inventario desde el almacen local.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErrorCarga {
    /// El fichero esta mal formado.
    #[error(transparent)]
    Formato(#[from] ErrorFormato),

    /// El fichero esta bien formado pero no supera la verificacion.
    #[error(transparent)]
    Verificacion(#[from] ErrorInventario),
}

/// Inventario cargado y verificado, listo para consultarse.
#[derive(Debug, Clone)]
pub struct InventarioLocal {
    inventario: Inventario,
    raiz: RaizVerificada,
}

impl InventarioLocal {
    /// Analiza y verifica el contenido de un fichero de inventario.
    ///
    /// # Errores
    ///
    /// [`ErrorCarga::Formato`] si el fichero esta mal formado —lo que se detecta
    /// **antes** de tocar criptografia— y [`ErrorCarga::Verificacion`] si la
    /// firma, el dominio de clave o la frescura no cuadran.
    pub fn cargar(
        bytes: &[u8],
        clave: &ClaveInventario,
        centinela: Centinela,
        revocaciones: &RegistroRevocaciones,
    ) -> Result<Self, ErrorCarga> {
        let fichero = analizar(bytes)?;
        let raiz = RaizVerificada::verificar(
            fichero.anclada,
            &fichero.firma,
            clave,
            centinela,
            revocaciones,
        )?;

        Ok(Self {
            inventario: fichero.inventario,
            raiz,
        })
    }

    /// Secuencia del inventario cargado, para avanzar el centinela.
    #[must_use]
    pub const fn secuencia(&self) -> u64 {
        self.raiz.secuencia()
    }

    /// Numero de marcados.
    #[must_use]
    pub fn entradas(&self) -> usize {
        self.inventario.marcados().len()
    }
}

impl ProveedorInventario for InventarioLocal {
    fn marcado(&self, mac: &DireccionEnlace) -> Result<Option<MarcadoVerificado>, ErrorProveedor> {
        let Some(posicion) = self.inventario.posicion_de(mac) else {
            // Ausencia legitima: el dispositivo no figura. Se distingue de un
            // fallo de verificacion, que RPT-010 §4 obliga a no confundir con
            // esto.
            return Ok(None);
        };

        let resumenes = self.inventario.resumenes();
        let prueba = prueba_inclusion(&resumenes, posicion, posicion as u64).ok_or_else(|| {
            ErrorProveedor::FuenteInaccesible {
                fuente: "prueba-de-inclusion".to_owned(),
            }
        })?;

        let marcado = self.inventario.marcados()[posicion];

        MarcadoVerificado::verificar_e_instanciar(marcado, &prueba, &self.raiz)
            .map(Some)
            .map_err(|error| match error {
                ErrorInventario::InclusionNoVerifica => ErrorProveedor::InclusionNoProbada,
                otro => ErrorProveedor::FirmaInvalida {
                    detalle: otro.to_string(),
                },
            })
    }
}
