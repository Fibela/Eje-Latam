# RPT-109 — La barrera que prohibía lo correcto

| | |
|---|---|
| **Tema** | La barrera que prohibía lo correcto |
| **Número** | 109 |
| **Fecha** | 2026-11-02 |
| **Área designada** | Método |
| **Abre** | — |
| **Cierra** | — (PA-165 sigue abierto: su criterio es una corrida entera en verde) |
| **Precedentes** | RPT-106 (la barrera que no corrió), RPT-011 §4, RPT-025 §2, RPT-086 §3 (las tres versiones de `rand_core`), RPT-005 §7.4 y PA-18 (fuente de aleatoriedad), RPT-003 §9.4 |

---

## 1. El hallazgo, en una frase

**La prohibición de `rand_core` en `deny.toml` prohibía el crate de rasgos en lugar
de los generadores peligrosos: bloqueaba trece dependencias correctas —dos de ellas
nuestras— y no atrapaba ni un peligro real.**

La regla:

```toml
deny = [
    { crate = "rand_core", wrappers = ["rand"],
      reason = "usar solo a traves de rand con backend criptografico" },
]
```

## 2. La medición

Quince crates del grafo dependen de `rand_core`. Sólo los dos `rand` estaban permitidos.

```
chacha20 0.10.1        crypto-common 0.1.7     crypto-common 0.2.2
getrandom 0.4.3        guardian-cc 0.1.0  ←     kem 0.3.0
ml-kem 0.3.2           motor-pqc 0.1.0    ←     password-hash 0.5.0
rand 0.10.2 ✓          rand 0.9.5 ✓            rand_chacha 0.9.0
signature 2.2.0        signature 3.0.0         x25519-dalek 2.0.1
```

Trece violaciones. **La regla prohibía a `motor-pqc` y `guardian-cc` hacer lo que
hacen.**

El fallo depende sólo de `Cargo.lock`: es determinista y no necesita red, avisos ni
versiones nuevas de nada.

### 2.1 La barrera llevaba fallando desde que se añadió

Corolario directo, y es el hallazgo de método:

> La compuerta no pasó de verde a rojo. **Lleva fallando desde que se escribió la
> regla, y era invisible porque la corrida ya salía roja por `Formato`.**

CI #3 es la primera corrida con todo lo demás en verde, y por tanto la primera en
que este fallo se puede ver. Los «ocho de nueve» no son una regresión: son la
primera vez que el proyecto puede mirar este problema.

Es el mismo patrón que RPT-106: una barrera cuyo estado real nadie había leído.
Allí no corría; aquí corría y fallaba, tapada por otro rojo.

## 3. Por qué la regla no decía lo que quería decir

Su `reason` busca evitar un generador no criptográfico en material de clave. Pero
`rand_core` **no genera nada**: define los rasgos `RngCore` y `CryptoRng`.

Depender de `rand_core` es precisamente cómo el ecosistema RustCrypto acepta un
CSPRNG de forma genérica. Lo hacen `ml-kem`, `signature`, `x25519-dalek` y
`chacha20` — y lo hace `motor-pqc`, que es lo correcto.

Los peligros reales son generadores concretos: `fastrand`, `oorandom`, `rand_pcg`,
`rand_xorshift`, `rand_hc`, `rand_isaac`, `nanorand`. Ninguno estaba prohibido.

**La regla atrapaba trece usos legítimos y cero peligros.**

## 4. Lo decidido

Prohibir por nombre los generadores no criptográficos y retirar la prohibición del
crate de rasgos. Verificado contra `Cargo.lock`: **ninguno de los siete está hoy en
el árbol**, de modo que la lista es una barrera hacia adelante, no un arreglo
retroactivo. El chequeo `bans` pasa.

Se descartaron dos alternativas:

| | Por qué no |
|---|---|
| Ampliar `wrappers` con los trece | Se rompe cada vez que cambia el grafo, y sigue sin describir ningún peligro |
| Documentar trece excepciones | Trece excepciones a una regla son la regla admitiendo que está mal |

## 5. Lo que esto **no** cierra

Tres cosas quedan fuera, y conviene que estén escritas para que nadie las dé por
resueltas:

1. **`SmallRng` vive dentro de `rand`**, que es dependencia legítima. Ninguna regla
   de `cargo-deny` lo atrapa. Queda para revisión de código o lint.
2. **La proliferación de versiones de `rand_core`** —hoy tres en el árbol: 0.6.4,
   0.9.5 y 0.10.1— es el peligro que RPT-011 §4, RPT-025 §2 y RPT-086 §3
   documentan como coste real y repetido. Sigue siendo sólo `warn`. Endurecerlo
   exige unificar el árbol primero.
3. **PA-18 sigue abierto**: *fuente de aleatoriedad, DRBG conforme al NIST frente a
   `OsRng`* (RPT-005 §7.4). La decisión sobre la aleatoriedad no la toma esta regla.

La regla retirada parecía una respuesta a (2) y (3) escrita como si fuera (1).
Retirarla no resuelve ninguna de las tres; deja de bloquear trece usos correctos.

## 6. Corrección de gobernanza

El encabezado de `deny.toml` dice que el fichero materializa **RPT-003 §2.7**. Al
leerlo: §2.7 es la ratificación de la **frontera open-core invertida** —Apache 2.0
definitivo para `guardian-cc`, `motor-pqc`, `eje-almacen` y `eje-red`, y la
desestimación del ocultamiento de binarios—. Y §9.4 manda `cargo deny` con un
propósito citado textualmente: *«Crítico para la frontera open-core: impide que una
dependencia copyleft contamine un crate Apache 2.0»*.

Es decir: **lo ratificado es el control de `[licenses]`. La prohibición de
`rand_core` en `[bans]` no tenía respaldo en ningún reporte** — `grep -rn rand_core
docs/` no devuelve ninguna decisión que la autorice, sólo las tres crónicas del
coste de sus versiones.

Por tanto este cambio **no es enmienda a una constancia firme**. Modifica una regla
que alguien añadió sin citar reporte. Queda escrita aquí para que lo tenga.

## 7. Dos defectos colaterales, sin resolver

Encontrados al medir, independientes de lo anterior:

1. **CI declara que valida sobre `stable` y valida sobre 1.85.**
   `rust-toolchain.toml` fija `channel = "1.85"`, y manda sobre el
   `dtolnay/rust-toolchain@stable` que el workflow declara **seis veces**. Con un
   `rust-toolchain.toml` presente, rustup lo honra por encima del toolchain por
   defecto. No rompe nada hoy: el fichero que documenta la barrera afirma una
   cobertura que la barrera no tiene.
2. **La rama por defecto sigue siendo `Origen`** (`refs/remotes/origin/HEAD`), que
   RPT-002 §2.1 ya retiró según RPT-004 §97. La rama viva es **`principal`**.

## 8. Nota de cadena de suministro

`motor-pqc` depende de `libcrux-ml-kem 0.0.10` y `libcrux-ml-dsa 0.0.10`. Las
versiones `0.0.x` no ofrecen garantía de estabilidad y son las más susceptibles de
retirada: la 0.0.3 de `libcrux-ml-kem` ya fue retirada. Con `yanked = "deny"` en
`[advisories]`, una retirada ahí **sí** pondría la compuerta en rojo sin que nadie
toque el repositorio. Hay una 0.0.11 publicada.

## 9. Lo que no se midió

**No se leyó el registro del trabajo de CI**: exige sesión autenticada en GitHub, y
el API de logs y el panel anónimo ambos la piden. La causa de §2 es la que la
configuración y el grafo hacen inevitable, no la línea de error leída.

Puede haber además un fallo de `licenses` encima: `deny.toml` excluye `MPL-2.0`
deliberadamente y no se pudieron comprobar las licencias de los 177 crates externos
sin `cargo`. El registro lo confirmaría en una lectura.

**PA-165 no se cierra con este reporte.** Su criterio es una corrida entera en
verde, y no se ha leído ninguna.

## 10. Apéndice de método — un verde cantado sin leer

Durante el diagnóstico se afirmó que el trabajo de `cargo-deny` *«pasó en verde en
la corrida #2»*. **Esa premisa nunca se midió.** De ella salió el razonamiento
«mismo `Cargo.lock`, verde→rojo en una hora, luego cambió algo externo», tres
hipótesis y la recomendación de relanzar el trabajo — que no habría cambiado nada.

Las tres hipótesis se midieron y cayeron:

| Hipótesis | Medición | Resultado |
|---|---|---|
| Aviso RustSec nuevo | El más reciente es RUSTSEC-2026-0420 (30-oct). Crates con aviso reciente cruzados contra `Cargo.lock` | Ninguno presente |
| Versión retirada | `libcrux-ml-kem`: única retirada es 0.0.3; se usa 0.0.10 | Limpio |
| `cargo-deny` nuevo vía `@v2` | Último release: 0.20.2, del 9-jul-2026 | Cuatro meses |

Cuando caen las tres hipótesis, lo que falla es el marco. El marco venía de una
premisa inventada.

Se registra porque es el tercer episodio del mismo error en esta línea de trabajo
—RPT-106 y el reporte de estado anterior son los otros dos— y en este caso lo
cometió el asistente, no el equipo. **La regla que lo evita no es «desconfía del
verde ajeno», es: ningún estado entra en un razonamiento sin que alguien haya leído
la corrida que lo afirma.**
