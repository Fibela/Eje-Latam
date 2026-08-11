# RPT-023 — Emisión de manifiestos (Diseño)

**Tema:** Que un administrador pueda firmar el inventario y las declaraciones de segmento
**Nº de reporte:** 023
**Fecha:** 6 de agosto de 2026
**Área designada:** Herramientas
**Entidad:** PremosCorp
**Estado:** **Diseño — sin implementar.** Requiere ratificación

- **Depende de:** RPT-011 (`DominioClave`), RPT-013 (formato), RPT-015 (revocación), RPT-022 (tabla de segmentos)
- **Cubre:** PA-48

---

## 1. Lo que falta es un emisor, no un formato

`serializar` existe, el formato está cerrado y las pruebas lo recorren de extremo a extremo. Lo que no existe es **nadie que produzca un `.inv`**. RPT-022 §9 lo dejó anotado antes de saber que sería lo siguiente, y es el mismo defecto de mecanismo-sin-cablear desplazado un nivel.

Sin esta herramienta, `DominioClave::Cliente` es un tipo sin titular: no hay clave de cliente porque no hay forma de crear una.

## 2. La herramienta que firma es la que puede falsificar

Primera decisión, y es de empaquetado antes que de código.

Si el emisor vive dentro del binario del agente, **cada sensor desplegado lleva encima la capacidad de firmar inventarios**. Un sensor está en el armario de planta o en el rack de la clínica, físicamente accesible, y su modelo de amenaza asume que puede caer. Toda la cadena de cinco eslabones de RPT-011 se apoya en que quien compromete el agente **no puede firmar**.

Luego: **crate y binario separados —`eje-manifiesto`— que no entra en el paquete de despliegue.** Es una regla de empaquetado (PA-12) tanto como de arquitectura, y conviene que una prueba la sostenga y no sólo un párrafo.

## 3. La clave privada no se serializa: se guarda una semilla

`ClaveFirmaHibrida` **no tiene `a_bytes`**, y la ausencia es deliberada. Añadirlo sería crear la función que permite al material privado salir del proceso, justo lo que RPT-021 §3 obliga a evitar para la clave de release.

No hace falta. `generar_par` es una función determinista del generador que recibe:

```rust
let privada_pq = PrivadaMlDsa::<MlDsa65>::generate_from_rng(generador);
let mut semilla = [0u8; 32];
generador.fill_bytes(&mut semilla);
```

Así que **basta persistir 32 bytes de semilla y re-derivar el par en cada ejecución**. El par vive en memoria lo que dura la firma y se destruye.

Dos condiciones que esto impone y que no son opcionales:

**El generador determinista debe ser un CSPRNG de verdad.** El `GeneradorDeterminista` de las pruebas es xorshift64\* con 64 bits de estado; usarlo aquí haría la clave adivinable. La expansión de semilla a flujo debe ser ChaCha20 (`rand_chacha`, ya presente en el árbol como dependencia transitiva).

**La semilla es la clave.** Cifrarla no cambia eso, sólo mueve el problema a lo que descifra.

## 4. Dónde vive la semilla — la decisión que hay que ratificar

`reposo::cifrar` toma una `ClaveSimetrica` de 32 bytes **ya derivada**. No hay derivación desde frase de paso en el workspace, así que hay tres caminos y ninguno es gratis:

| Opción | Qué añade | Qué cuesta |
|---|---|---|
| **A. Argon2id desde frase** | dependencia nueva (`argon2`) | la seguridad baja al nivel de la frase que elija el administrador |
| **B. Fichero de clave con permisos del sistema** | nada | quien lee el disco lee la clave; en Windows los permisos son más frágiles de lo que parecen |
| **C. Token o HSM, como PA-14a** | nada de código | compra, y para la clave del **cliente**, no de PremosCorp |

Mi lectura: **A para la operativa, C para la de recuperación.** La operativa se usa cada vez que cambia el parque —semanas, no años— y la fricción de un token en cada emisión reproduce el error que RPT-021 §6 rechazó para la firma de release. La de recuperación se usa una vez en años, sólo firma certificados de revocación, y allí la fricción **es** el objetivo (RPT-015 §4, PA-32).

No la doy por decidida: la opción B es defendible en un piloto y quiero que se rechace explícitamente si se rechaza.

## 5. La secuencia se lee del manifiesto anterior

Un fichero de estado aparte se pierde, y perderlo hace que el emisor reinicie en 1 — con lo que el agente rechaza por reversión todo lo que venga después. El manifiesto anterior ya lleva la secuencia dentro y es el único sitio donde no puede desincronizarse.

Pero eso convierte al manifiesto anterior en **entrada**, y hay dos trampas:

**Verificar antes de creer.** Si el emisor toma la secuencia sin comprobar la firma, quien edite el `.inv` del administrador decide qué secuencia se emite a continuación.

**Techo de emisión.** Un manifiesto manipulado a `u64::MAX - 1` haría que el emisor produjera `u64::MAX`, y ése es exactamente el bloqueo permanente de PA-33: ningún inventario legítimo puede ya superarlo y sólo un certificado de revocación lo deshace. El emisor debe **negarse a emitir por encima de un techo razonable** y decir por qué. Un contador que sube de uno en uno no llega a 2⁶⁴ por uso legítimo; llegar ahí es señal, no crecimiento.

## 6. Qué escribe el administrador

Un TOML con marcados y segmentos, en la línea de `contrato-contencion.toml`. Pero el workspace **no tiene analizador de TOML**: los manifiestos existentes se leen a mano con búsqueda de subcadenas, que sirve para una prueba de paridad y no para entrada de usuario.

Así que es otra dependencia a ratificar (`toml` + `serde`, este último ya presente en `eje-ipc`). La alternativa —un formato propio de líneas— evita la dependencia y crea un analizador más que auditar. **Recomiendo la dependencia**, con `deny_unknown_fields`: un campo mal escrito que se ignora en silencio es cómo un marcado de soporte vital acaba emitido como no crítico.

## 7. Lo que este diseño no resuelve

1. **El agente no puede recibir la clave de verificación.** `eje-agente` opera en primer arranque porque no hay aprovisionamiento. Emitir manifiestos firmados no sirve de nada si el sensor no sabe con qué clave verificarlos. Se registra como **PA-49**, y es la mitad que falta del mismo problema.
2. **La clave del cliente vivirá en un portátil.** No es un HSM y no vamos a fingir que lo es. Lo que sí puede hacer el producto es que su compromiso sea **recuperable** —para eso existe RPT-015— y que la ventana sea acotable por revocación.
3. **Nadie ha decidido quién es el administrador.** RPT-021 §7 dejó la custodia de la clave de release sin procedimiento; aquí pasa lo mismo un nivel más abajo, y con más gente implicada porque cada cliente tiene la suya.
4. **La rotación programada no está.** RPT-015 da el mecanismo de revocación, que es para emergencias. Rotar por higiene es otra cosa y nadie la ha escrito.

El punto 1 es el que convierte todo esto en la mitad de una función.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-48** | **Emisor de manifiestos.** Este reporte | Que RPT-022 tenga usuarios |
| **PA-49** | **Aprovisionamiento de la clave de verificación en el agente.** Sin él, un manifiesto firmado no se puede comprobar en el sensor | Que RPT-011 tenga usuarios |
| PA-50 | Procedimiento de custodia y rotación de la clave del cliente | Continuidad más allá del piloto |

---

## 9. Qué se pide ratificar

1. **§2** — crate y binario separados, fuera del paquete de despliegue, con prueba que lo sostenga.
2. **§3** — semilla de 32 bytes en lugar de clave serializada, con ChaCha20 como expansor.
3. **§4** — Argon2id para la operativa, token o custodia externa para la de recuperación. O el rechazo explícito de esa división.
4. **§5** — secuencia leída del manifiesto anterior **verificado**, con techo de emisión.
5. **§6** — dependencia de `toml` con `deny_unknown_fields`.

---

*Reporte Nº 23 — Emisión de manifiestos (Diseño) · PremosCorp · 6 de agosto de 2026*
