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

---

## 11. Addendum tras CI #4 — lo medido después de publicar

### 11.1 CI #4 (`150c878`): ocho de nueve, mismo trabajo rojo

Verde: `Formato`, `Análisis estático`, `Pruebas` (ubuntu y windows), `Implementaciones
inconclusas`, `Índices derivados`, `Secretos expuestos`, `SBOM`.
Rojo: **`Licencias y vulnerabilidades`**, 58 s. Idéntica forma que #3.

**El arreglo de §4 no es atribuible.** `bans` pudo quedar corregido y fallar
`licenses` o `advisories` encima, o `bans` puede seguir fallando por otra causa.
Sin el registro no se distingue, y no se razona por encima de una medición que falta.

Acotado en local, sin `cargo`:

- **No hay ninguna versión con comodín** → `wildcards = "deny"` está satisfecho.
- **Ningún crate de licencia no permitida de los habituales** está en el árbol.
- `shlex` está en 1.3.0 y 2.0.1; **1.3.0 ya es la versión corregida** de su aviso.

Queda como candidato principal una **retirada entre los crates muy nuevos**
—`core-models 0.0.7`, `shake 0.1.0`, `sponge-cursor 0.1.0`, la familia
`libcrux-* 0.0.x`, `ml-dsa 0.1.1`— bajo `yanked = "deny"`. **Es hipótesis, no
medición.**

### 11.2 Corrección a §4: la acción no puede leer `rust-toolchain.toml`

Medido sobre `action.yml` de `dtolnay/rust-toolchain`: el campo `toolchain` es
`required: true` y el primer paso **aborta explícitamente** si llega vacío
(`"'toolchain' is a required input"`, exit 1). La acción **no** lee
`rust-toolchain.toml`.

Eso descarta la solución aparentemente obvia —dejar el fichero como fuente única y
que la acción lo obedezca—: habría puesto CI en rojo en los seis trabajos.

**Consecuencia nueva, más concreta que «el fichero miente».** La acción ejecuta
`rustup toolchain install stable` y `rustup default stable`, y publica como salida
`cachekey` el hash de **rustc stable**. `Swatinem/rust-cache@v2` indexa con eso.
Después, `cargo` en el directorio encuentra `rust-toolchain.toml`, y rustup
**autoinstala 1.85** para obedecerlo. Resultado:

1. Se descarga un toolchain de más en cada trabajo.
2. **La clave de caché describe un toolchain que no es el que compila.**

Así que el defecto no es sólo documental: cuesta descargas y degrada la caché.

**Forma del arreglo, verificada:**

```yaml
- uses: dtolnay/rust-toolchain@master
  with:
    toolchain: "1.85"
```

`dtolnay/rust-toolchain@1.85.0` **no existe** como etiqueta (comprobado contra el
API de refs), de modo que `@master` es la única vía — y es una referencia
flotante, la misma clase de problema que el `@v2` de `cargo-deny-action`. Conviene
fijarla a un SHA.

### 11.3 Por qué no se aplicó

**CI está en rojo por una causa que no se ha leído.** Tocar los seis trabajos que
hoy están verdes crearía un segundo rojo sin explicar y haría imposible atribuir
cualquiera de los dos. Es el modo de fallo que §10 describe, cometido a sabiendas.

El arreglo entra **después** de que el registro de `cargo-deny` esté leído y la
compuerta tenga una causa con nombre. Y la pregunta de fondo —1.85, `stable` o el
1.97.0 de §6— es una decisión del equipo, no una consecuencia técnica de esto.

### 11.4 Corrección a §7.2: son tres nombres de rama, no dos

§7.2 decía que la rama por defecto es `Origen` y la viva `principal`. Incompleto.
`ci.yml` §6–14 documenta un tercero:

- **`principal`** (minúscula) — donde vive el desarrollo.
- **`Principal`** (mayúscula) — **commit raíz HUÉRFANO** del 4 de agosto (LICENSE,
  README y RPT-002, sin antepasado común con el trabajo). Durante **77 días** el
  workflow sólo nombraba este, y por eso **no se ejecutó ni una vez sobre los 67
  commits siguientes**. Es la causa original de PA-165.
- **`Origen`** — la rama por defecto en GitHub, que RPT-002 §2.1 retiró.

El workflow nombra hoy los dos primeros a propósito, con el argumento correcto:
*«una barrera que depende de la mayúscula de un nombre no es una barrera»*.
Consolidar los nombres sigue pendiente, y ahora son tres.
