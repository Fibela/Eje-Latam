# RPT-026 — Emisor de manifiestos (segunda mitad)

**Tema:** Que un administrador pueda usarlo sin escribir Rust
**Nº de reporte:** 026
**Fecha:** 6 de agosto de 2026
**Área designada:** Herramientas
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-48

- **Depende de:** RPT-023 §§4 y 6 (ratificados), RPT-025 (mitad criptográfica)
- **Cierra:** PA-48
- **Abre:** PA-53, PA-54

---

## 1. `deny_unknown_fields` es la razón entera de traer un analizador

El workspace no tenía analizador de TOML: los manifiestos existentes se leen a mano con búsqueda de subcadenas, que sirve para una prueba de paridad y no para entrada de usuario.

La razón de traer `toml` + `serde` no es comodidad. Es esto:

```toml
[[marcado]]
mac = "00:1b:21:00:00:01"
clse = "soporte-vital"
```

Sin `deny_unknown_fields`, ese fichero produce un marcado **no crítico** de un equipo de soporte vital, porque `clase` está ausente y su ausencia significa «declarado no crítico». El administrador no tiene ninguna forma de notarlo hasta el incidente.

Un analizador por subcadenas habría hecho exactamente eso: ignorar lo que no reconoce.

La misma regla se extiende a los valores, no sólo a las claves: `clase = "soporte-vitall"` se rechaza en lugar de degradarse a no crítico. Un vocabulario cerrado que acepta lo desconocido no es cerrado.

## 2. La MAC exige separadores

`001b21000001` se rechaza. Doce caracteres seguidos se transponen sin notarlo, y **una MAC transpuesta marca el equipo equivocado** — no falla, protege a otro. Con `:` o `-` el ojo tiene puntos de anclaje.

También se comprueba que cada octeto sean dos dígitos hexadecimales, aparte de `from_str_radix`: ese acepta signo, así que `+1` mide dos caracteres y valdría 1.

## 3. La semilla en reposo

Argon2id sobre la frase de paso, AES-256-GCM sobre la semilla, según RPT-023 §4. Tres decisiones que no venían en el diseño:

**La cabecera se autentica.** Mágico, versión y sal viajan como datos asociados del AEAD. Sin eso, cambiar la sal de un fichero ajeno y observar cómo falla el descifrado sería un principio de oráculo. Autenticada, cualquier alteración produce el mismo fallo indistinguible — que es el mismo criterio por el que `reposo::descifrar` no dice cuál de sus entradas falló.

**El nonce es por cifrado, no por fichero.** Como la clave se deriva de (frase, sal), reescribir con la misma frase y la misma sal daría la misma clave, y reutilizar nonce con la misma clave rompe GCM por completo. Se generan sal y nonce nuevos en cada escritura, que es la vía barata de no depender de acordarse.

**La frase vacía se rechaza al crear**, no sólo al abrir. Cifrar con frase vacía es la opción B que RPT-023 §4 rechazó, disfrazada de cifrado.

## 4. Dos cosas que el binario hace y merecen mención

**Se niega a sobrescribir una semilla.** Pisarla deja huérfano todo lo firmado con la anterior, y el agente lo leerá como firma inválida — es decir, como **manipulación**. Un `--force` aquí sería una palanca para convertir un despiste en una alerta de incidente.

**Avisa de que la clave de recuperación falta.** `generar` produce la operativa y aprovisiona su `.pub`, pero no la de recuperación. Decirlo por pantalla es lo mínimo: sin ella no se pueden leer certificados de revocación, que es el único remedio si la semilla se compromete.

## 5. La frase de paso se ve al teclearla

Ocultarla exige otra dependencia para manejar el terminal, y traerla junto a Argon2id, TOML y la aleatoriedad del sistema habría mezclado cuatro APIs sin verificar en un solo paso.

Se lee de la entrada estándar y **no de una variable de entorno**: en varios sistemas el entorno de un proceso es legible por otros usuarios, y en casi todos acaba en el historial del intérprete. Que se vea en pantalla es peor que ocultarla y mejor que dejarla en el entorno.

Va como **PA-53** y el programa lo avisa. Un aviso no es una solución, pero una limitación anunciada se puede evitar; una silenciosa no.

## 6. Sin respaldo para la aleatoriedad

Si el sistema no entrega bytes aleatorios, el programa falla y no genera nada. **No hay generador de reserva.** Uno escrito por nosotros sería peor que fallar, porque produciría claves con la apariencia de buenas y nadie volvería a mirar.

## 7. Lo que sigue sin resolverse

1. **La clave de recuperación no se genera.** RPT-015 §4 la exige separada y con custodia distinta —token o material fuera de línea—, y este binario no la produce a propósito: generarla en el mismo comando y guardarla junto a la operativa anularía la separación. Es **PA-54**.
2. **La vigencia por defecto está duplicada.** `VIGENCIA_POR_DEFECTO` vale 365 aquí y `clasificacion.vigencia_marcado_dias` vale 365 en `contrato-contencion.toml`. Debería haber una sola fuente y no la hay, porque este crate no lee el manifiesto. Es deuda pequeña y del tipo que este proyecto lleva veinticinco reportes persiguiendo, así que queda escrita.
3. **No hay prueba de extremo a extremo del binario.** Las pruebas cubren la biblioteca —incluido el ciclo completo de frase de paso a veredicto—, pero nadie ejecuta el ejecutable con argumentos. Eso son pruebas de proceso y las trae PA-12.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-53** | **Lectura de la frase de paso sin eco.** Hoy se ve al teclearla | Uso delante de terceros |
| **PA-54** | **Generación y custodia de la clave de recuperación.** No puede salir del mismo comando que la operativa | El remedio ante compromiso de la semilla |
| PA-48 | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 26 — Emisor de manifiestos (segunda mitad) · PremosCorp · 6 de agosto de 2026*
