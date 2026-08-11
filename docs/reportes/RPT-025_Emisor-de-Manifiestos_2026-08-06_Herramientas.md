# RPT-025 — Emisor de manifiestos (primera mitad)

**Tema:** Que alguien pueda producir un inventario firmado
**Nº de reporte:** 025
**Fecha:** 6 de agosto de 2026
**Área designada:** Herramientas
**Entidad:** PremosCorp
**Estado:** **Implementado parcialmente.** PA-48, mitad criptográfica

- **Depende de:** RPT-023 (diseño ratificado), RPT-024 (formato de clave aprovisionada)
- **Cubre:** PA-48 §§2, 3, 5. **No cubre** §§4 y 6 — ver §6 de este reporte
- **Cierra además:** el ataque de PA-33 por la puerta del fichero

---

## 1. `rand_chacha` no sirve, y hay que decirlo

RPT-023 §3 quedó ratificado con ChaCha20 como expansor de semilla. Al ir a añadir la dependencia apareció que **no puede funcionar**: `rand_chacha 0.9` implementa los rasgos de `rand_core 0.9`, y `generar_par` exige el `CryptoRng` de `rand_core 0.10`. La firma del genérico no encaja.

Meter otra versión de `rand_core` en este árbol tiene precedente y no bueno — el comentario de `guardian-cc/Cargo.toml` dice literalmente que tres versiones conviviendo ya costaron una sesión.

El expansor se construye sobre `sha2`, que ya es dependencia de `motor-pqc`. La construcción es **MGF1** —`SHA-256(etiqueta ‖ semilla ‖ contador)`, con los campos prefijados en longitud como en el resto del proyecto—, que es la familia que PKCS#1 estandariza para expandir una semilla en un flujo.

Tiene una ventaja que no buscaba y conviene registrar: **ninguna llamada es falible.** Con HKDF habría un `Result` que no puede fallar con longitudes de 32 bytes, y eso obliga a escribir un `expect` —prohibido por las lindes del proyecto— en la ruta que produce material de clave. Con SHA-256 directo no hay nada que desenvolver.

El alcance está acotado en el propio tipo: sirve para **re-derivar** un par de una semilla que ya tiene 256 bits de entropía, no para **crear** esa semilla. Eso sigue exigiendo el generador del sistema.

## 2. El techo de secuencia estaba en el sitio equivocado

RPT-023 §5 lo ratificó en el emisor. Al implementarlo se vio que ahí no protege de nada: **quien tenga la clave no usa nuestro emisor.** El bloqueo permanente de PA-33 ocurre en el agente, cuando acepta un inventario con secuencia `u64::MAX` y ningún manifiesto legítimo puede ya superarlo.

Así que el techo vive ahora en `formato::analizar`, **antes de tocar criptografía**, y devuelve `ErrorFormato::SecuenciaFueraDeRango`. Es la puerta por la que llega todo inventario real, así que cierra el ataque en la entrada en lugar de dejarlo a la recuperación por certificado.

El orden importa: si se comprobara después de la firma, un inventario saturado y correctamente firmado sería «válido pero rechazado», y esa distinción invita a que alguien la relaje.

La comprobación del emisor se mantiene, degradada a lo que es: una cortesía para no producir un fichero que el sensor va a rechazar.

**Lo que esto no cubre**, y queda escrito para que no se confunda una defensa de perímetro con un invariante: `RaizVerificada::verificar` —el paso en memoria— no lleva el techo. Hoy nadie construye una `RaizAnclada` fuera del analizador salvo las pruebas. Mañana puede que sí.

## 3. La secuencia sale del manifiesto anterior verificado

Un fichero de estado con el último número se pierde, y perderlo hace que el emisor reinicie en 1: el agente rechazaría por reversión todo lo que viniera después. El manifiesto anterior ya lleva la secuencia dentro y es el único sitio donde no puede desincronizarse.

Pero eso lo convierte en **entrada de un fichero manipulable**, con dos trampas que las pruebas fijan:

- **Verificar antes de creer.** `secuencia_siguiente` carga el anterior con `InventarioLocal::cargar`, que comprueba firma y dominio de clave. Un emisor con otra semilla no puede continuar la serie, y eso es lo correcto: no es su serie.
- **Un anterior corrupto no reinicia.** Si el fallo devolviera 1, bastaría corromper el fichero para que el siguiente manifiesto naciera revertido. Peor: el administrador creería haber emitido y no lo sabría hasta el incidente.

La primera emisión es la **1** y no la 0, para que «sin manifiesto» y «manifiesto inicial» no compartan número.

## 4. El emisor usa el lector del agente

`Inventario::construir` y `TablaVlan::construir` son el **mismo código** que corre en el sensor. Escribir un constructor propio en el emisor permitiría que las dos mitades discreparan sobre qué es un manifiesto válido, y esa discrepancia se descubriría en planta.

Como efecto, el emisor rechaza al escribir lo que el agente rechazaría al leer: dispositivo duplicado, VLAN fuera de rango, inventario vacío. Un manifiesto que el sensor va a rechazar no debería llegar a existir.

## 5. Lo que la separación de crates sí y no garantiza

`eje-manifiesto` es un crate aparte para que un sensor comprometido no lleve encima la capacidad de firmar inventarios. Hay una prueba, `el_emisor_no_entra_en_el_binario_del_agente`, y conviene decir exactamente qué comprueba: que `eje-agente/Cargo.toml` **no lo declara como dependencia**.

Eso no impide que el empaquetador copie el binario del emisor al instalador. Sólo lo cierra una comprobación sobre el artefacto, y esa es **PA-12** y no existe. La prueba de aquí es necesaria y no suficiente, y presentarla como suficiente sería el tipo de garantía de papel que este proyecto lleva veinte reportes desmontando.

## 6. Lo que falta de PA-48

Esta es la mitad criptográfica. Queda la mitad de entrada y salida, que exige las dos dependencias nuevas que RPT-023 ratificó y que no se han añadido:

1. **`toml` + `serde` con `deny_unknown_fields`.** Lo que escribe el administrador. Sin esto no hay binario ejecutable: hoy `eje-manifiesto` es una biblioteca sin `main`.
2. **`argon2` para la semilla en reposo.** Hoy la semilla se pasa en memoria y nadie la persiste. Un emisor que exija reintroducir 32 bytes en cada uso no lo usará nadie, así que esto no es opcional: es la diferencia entre una herramienta y una demostración.

Se separan a propósito. Las dos añaden superficie de dependencia que no puedo verificar contra el compilador en el mismo paso que la lógica criptográfica, y mezclarlas haría que un fallo de API de `argon2` pareciera un fallo del expansor.

## 7. Deuda que este reporte deja

1. **Nadie crea la semilla.** `derivar_par` la consume; producirla exige el generador del sistema y no hay código que lo haga. Va con el punto 2 del §6.
2. **El emisor no aprovisiona.** Produce la clave de verificación y `guardian-cc` sabe escribirla, pero nadie encadena las dos cosas. Es PA-51.
3. **`RaizVerificada::verificar` sigue sin techo.** Ver §2.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-48** | **Mitad de entrada/salida del emisor**: TOML, Argon2id, binario y creación de semilla | Que el emisor sea usable |
| PA-52 | Techo de secuencia en el camino en memoria, no sólo en el analizador | Nada hoy; deuda de invariante |

---

*Reporte Nº 25 — Emisor de manifiestos (primera mitad) · PremosCorp · 6 de agosto de 2026*
