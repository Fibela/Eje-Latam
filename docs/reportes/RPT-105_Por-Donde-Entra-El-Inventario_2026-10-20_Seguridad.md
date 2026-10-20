# RPT-105 — Por dónde entra el inventario

| | |
|---|---|
| **Tema** | Por dónde entra el inventario (propuesta de cierre de PA-162) |
| **Número** | 105 |
| **Fecha** | 2026-10-20 |
| **Área designada** | Seguridad |
| **Naturaleza** | **Propuesta.** Nada de esto está construido. Se escribe antes de tocar código |
| **Cierra** | PA-162 (parcialmente — ver §9) |
| **Precedentes** | RPT-104 (PA-162), RPT-012 §4.4 (reversión), PA-150, PA-161 |

---

## 1. Objetivo

> **Que un sensor aprovisionado desde cero alcance `EstadoArranque::Operativo`
> con su inventario firmado aceptado, sin que exista ningún momento en el que un
> inventario antiguo y correctamente firmado pueda entrar.**

Una sola frase, y las dos mitades son obligatorias. La primera sin la segunda se
consigue en diez minutos y abre el ataque de reversión. La segunda sin la primera
es lo que tenemos hoy.

**Criterio de éxito, medible y único:** la calma de RUA sale con
`condiciones=ninguna` desde una golden aprovisionada por la ceremonia. Mientras
eso no se vea en el colector, esto no está cerrado — y no vale ninguna prueba
unitaria en verde como sustituto, porque las pruebas de `guardian-cc` ya están
verdes hoy y PA-162 existe.

---

## 2. El estado de hoy, medido

De RPT-104 §4, y todo esto está observado, no deducido:

- `aceptar_inventario` es la única función que avanza la marca de agua de
  inventario. **No la llama nadie en producción**, sólo las pruebas de
  `guardian-cc`.
- Los seis canales IPC son de **lectura**: `ObtenerEstadoAgente`,
  `ObtenerInventario`, `ObtenerEstadoBoveda`, `ConsultarSandbox`,
  `ConsultarAlertas`, `ObtenerCondiciones`. Ninguno escribe nada.
- Con la marca en «sin establecer», `RaizVerificada::verificar` sale por
  `FrescuraNoEstablecida` **antes** de comparar firma o secuencia.
- Los dos caminos están cerrados: con `inventario.inv` colocado a mano,
  `FrescuraNoEstablecida` para siempre; sin colocarlo, `PrimerArranque` para
  siempre.

Y un hecho que hace barata la solución: **`eje-manifiesto` ya depende de
`guardian-cc`**. Puede llamar a `aceptar_inventario` hoy mismo.

---

## 3. La restricción que decide, y por qué la elección no es libre

El primer inventario tiene el mismo problema de arranque en frío que el ancla de
identidad, y conviene verlo antes de elegir.

Para fechar un inventario hace falta una marca de agua. Para tener una marca de
agua hace falta haber aceptado un inventario. El primero, por definición, no
tiene contra qué compararse — y por eso existe `FrescuraNoEstablecida`, que dice
exactamente eso: *«no hay centinela de frescura; el inventario no puede
fecharse»*.

La pregunta no es cómo saltarse esa comprobación. Es **quién tiene autoridad para
decir «este es el primero»**. Porque quien la tenga puede, por construcción,
introducir cualquier inventario firmado sin que nada lo feche.

PA-150 ya contestó esta pregunta para las anclas de identidad, y la contestó así:
**el agente no ancla nunca**, porque anclar al primer uso convierte el ataque en
«borro los dos ficheros y dejo la mía». PA-161 lo materializó: lo hace
`eje-manifiesto`, en una ceremonia, con un administrador delante.

La marca de inventario es la misma clase de objeto. Si el agente la establece por
su cuenta —aceptando lo que encuentre en disco al arrancar, por ejemplo— entonces
borrar el centinela y plantar un inventario viejo y bien firmado es un ataque de
reversión que funciona, y `FrescuraNoEstablecida` deja de significar «alguien
borró el centinela» para significar «empecemos de nuevo con lo que haya».

> **La restricción, en una línea:** `FrescuraNoEstablecida` tiene que seguir
> siendo alcanzable **sólo** cuando alguien borró el centinela de verdad. Toda
> opción que la convierta en un estado del que el agente sale solo está mal, por
> cómodo que resulte.

---

## 4. Las tres opciones, y por qué dos se caen

### Opción A — el agente acepta al arrancar lo que encuentre. **Rechazada.**

Es la de una línea de código. Y es exactamente el ataque descrito arriba: el
agente se convierte en la autoridad que declara «este es el primero», sin nadie
delante, en cada arranque. Reintroduce la forma de PA-150 en el subsistema
vecino, seis semanas después de haberla cerrado.

### Opción B — un canal IPC de escritura que acepte inventarios. **Rechazada por ahora, y no por el motivo que parece.**

Es la opción que yo mismo propuse de pasada antes de leer el código, y estaba
mal. El problema no es el esfuerzo: es que **la autorización que haría falta no
existe**.

`autorizar(canal, longitud_carga)` comprueba dos cosas: que el nombre del canal
sea conocido y que la carga no pase de `LONGITUD_MAXIMA_MARCO`. **No autentica a
quien llama.** El control de acceso real es el grupo del socket (`grupo_ipc`), es
decir permisos de sistema de ficheros.

Para los seis canales de lectura eso es razonable: el panel lee estado. Para un
canal que **establece la marca de frescura**, no lo es en absoluto — convertiría
a cualquier proceso local del grupo IPC en la autoridad que decide qué inventario
es el primero. Sería entregarle el ataque de reversión a un adversario que sólo
necesita estar en un grupo.

Y hay una consecuencia de diseño que merece decirse: **sería el primer canal de
escritura en la historia de este IPC.** Los seis que hay son `Obtener*` y
`Consultar*`. Eso no es una ampliación incremental, es una frontera de confianza
nueva, con su autenticación, su cadena de paridad de contrato
(`contrato-ipc.toml` ↔ `CAMPOS_*` ↔ struct ↔ `PuenteEje` ↔ `vis04.ts`) y su
política de qué pasa si llega dos veces. No puede viajar de polizón en el arreglo
de un bloqueante.

### Opción C — la ceremonia siembra el primero. **Propuesta.**

El administrador que acaba de generar las claves, emitir el inventario, firmar la
configuración y anclar la identidad **ya es la autoridad de confianza de ese
sensor**, y está delante. Que en ese mismo acto quede establecida la marca del
inventario que él acaba de emitir no añade ninguna autoridad nueva al sistema:
usa la que ya existe, en el único momento en que existe.

Es barata (`eje-manifiesto` ya depende de `guardian-cc`), es simétrica con
PA-161, y no toca el IPC.

---

## 5. La propuesta

### 5.1 Orden nueva: `eje-manifiesto sembrar`

```
eje-manifiesto sembrar --almacen <dir>
```

**Nombre propuesto, a confirmar antes de escribirlo.** No lo doy por fijado: si
el equipo prefiere otro, se cambia aquí primero y luego en el código, no al
revés.

Qué hace: lee el `inventario.inv` del almacén, obtiene su secuencia, y llama a
`aceptar_inventario` para que escriba el centinela con la marca de inventario
establecida en esa secuencia — centinela primero, inventario después, que es el
orden que ya tiene esa función y que existe para que no quede nunca un inventario
cuya secuencia no conste.

**Guardas, y cada una responde a un ataque concreto:**

| Guarda | Qué impide |
|---|---|
| Exige que **ya haya centinela** | Sembrar sin identidad anclada dejaría un sensor que fecha inventarios y no sabe quién es. El orden de la ceremonia es `anclar` → `sembrar` |
| **Se niega si la marca de inventario ya está establecida** | Es la guarda de reversión. Volver a sembrar sobre un sensor que ya vivió bajaría su marca a la del inventario que haya en disco, y eso es el ataque servido por la herramienta |
| Exige que el `inventario.inv` **verifique** con la clave operativa anclada | Sembrar un inventario que el agente va a rechazar produce un sensor sembrado y roto, y el administrador se enteraría al arrancar en vez de al sembrar |
| **No toca la marca de configuración** | RPT-078. `aceptar_inventario` ya relee el disco y funde, precisamente para no borrar la otra marca en silencio. Se apoya en eso, no se reimplementa |

### 5.2 Por qué orden nueva y no ampliar `anclar`

`anclar` deja las dos marcas **sin establecer** porque el sensor no ha vivido
nada. `sembrar` establece una porque acaba de aceptar algo. Son actos distintos
sobre el mismo fichero, y PA-161 ya razonó este caso exacto al decidir no
ampliar `migrar-centinela`: **juntar dos órdenes de las que una preserva y otra
no da una orden que decide por lo que encuentre en disco.**

Hay además un motivo práctico: se puede querer anclar identidad sin tener todavía
un parque que inventariar. Separadas, eso es una ceremonia válida a medias.
Juntas, es un error.

### 5.3 La ceremonia queda así

```
generar → recuperacion → emitir(parque.toml) → configurar(sensor.toml)
        → anclar → sembrar
```

Seis pasos. El sexto es nuevo y `docs/Comandos.md` tiene que documentarlo, o
`cargo xtask manual` se pone rojo — que es su trabajo.

---

## 6. Qué recibe RUA, exactamente

Esto importa operativamente y conviene que no se improvise el día que toque.

Tras sembrar, el único artefacto que cambia es **`centinela.dat`**: sigue
midiendo 94 bytes, pero su marca de inventario ya no está «sin establecer».
`inventario.inv` se reescribe con los mismos bytes, así que **su SHA-256 sigue
siendo `23fffe6e…`**. Las dos claves públicas y `agente.conf.firmado` no se
tocan.

Luego el procedimiento de RUA es:

1. Restaurar `d520dd87`.
2. Sustituir **sólo** `centinela.dat` en `/var/lib/eje-latam`, con verificación
   de SHA en destino.
3. `sync` + `savestate`, y snapshot nuevo — `base-inventario-sembrado`.
4. Restaurar `atacante-pa61` a `base-dispositivo`, levantar las dos.
5. `systemctl start eje-agente`, leer del sink.

`d520dd87` **se conserva**. Es el baseline de «identidad anclada, inventario
imposible», y es la única forma de volver a demostrar que PA-162 existía.

---

## 7. Pruebas de fuego, con las predicciones escritas antes

Las predicciones van aquí, en el documento de propuesta, **antes de que exista el
código**. Si alguna falla, se reporta que falló; no se reescribe.

| # | Qué se prueba | Predicción |
|---|---|---|
| 1 | Almacén recién `anclar`-ado, `sembrar`, luego `arrancar` | `EstadoArranque::Operativo`. Es la prueba que PA-162 convierte en imposible hoy |
| 2 | Tras la 1, `condiciones` del agente | `inventarioNoVerifica = false`, `inventarioSuprimido = false`, y **las dos de aprovisionamiento también en falso** (las anclas las puso `anclar`) |
| 3 | `sembrar` sobre un almacén **sin** centinela | Se niega. No escribe nada |
| 4 | `sembrar` dos veces | La segunda se niega por marca ya establecida, y el centinela **no cambia** — se comprueba el contenido del fichero, no el texto del error |
| 5 | `sembrar` con un `inventario.inv` firmado por otra clave | Se niega. El sensor no queda sembrado |
| 6 | Tras sembrar con secuencia N, presentar un inventario de secuencia N−1 | `ReversionDetectada { aceptada: N, presentada: N-1 }`. **Esta es la prueba de que el objetivo §1 se cumplió**: la reversión que hoy es inalcanzable —porque nunca hay marca— pasa a detectarse |
| 7 | `sembrar` y comprobar la marca de **configuración** | Intacta. La guarda de RPT-078 sigue en pie |

**Sobre el sello del latido no predigo nada todavía.** RPT-104 §6.1 me obliga:
antes de decir qué `sello=` llegará al colector en calma con inventario aceptado,
hay que leer en `ciclo.rs` cuándo se anota un asiento en el registro de
evidencia. Es la segunda vez que predigo un campo de syslog sin leer su cadena de
formato, y no va a haber una tercera. La predicción del sello se escribe cuando
se haya leído, y antes de la corrida.

---

## 8. Perturbaciones obligatorias

Ninguna de las siete pruebas cuenta hasta que se la ha visto ponerse roja. En
este proyecto una prueba de guarda ya pasó en verde estando mal (RPT-104 §2.1), y
el motivo fue comprobar un texto en vez de un efecto.

| Perturbación | Qué tiene que ponerse rojo |
|---|---|
| Retirar la guarda de «ya hay marca» | La prueba 4, **por el centinela cambiado**, no por el mensaje |
| Retirar la guarda de «hay centinela» | La prueba 3 |
| Retirar la verificación del inventario | La prueba 5 |
| Sembrar con `secuencia + 1` en vez de la real | La prueba 6, porque `aceptada` no cuadraría |
| Escribir el inventario antes del centinela | Ninguna prueba actual. **Si es así, falta una prueba** y hay que escribirla antes de dar esto por cerrado |

La última fila es la importante: está puesta para descubrir un hueco, no para
confirmar un acierto. Si sale que ninguna prueba nota la inversión del orden de
escritura, eso es un hallazgo y se anota.

---

## 9. Qué NO hace esta propuesta

Decirlo es parte de la propuesta, porque un cierre que parece total y no lo es
produce la siguiente PA-162.

**No resuelve cómo entra el segundo inventario.** Un parque cambia: se añade una
máquina, se reclasifica otra. Con esto, actualizar el inventario de un sensor
exige volver a la ceremonia y tocar el almacén. Para un laboratorio y para los
primeros despliegues es aceptable. Para un producto no lo es.

Ese camino necesita un canal de escritura en el IPC, y **el IPC no tiene
autenticación de quien llama** (§4, opción B). Eso es un punto propio que hay que
abrir, con su propio diseño: autenticación, idempotencia, qué pasa si llega un
inventario mientras el agente está sirviendo otra petición, y la cadena de
paridad de contrato completa. No cabe aquí y no debe caber.

> **Se propone abrir PA-164:** «el inventario sólo se puede actualizar volviendo
> a la ceremonia; el IPC no tiene canal de escritura ni autenticación de
> llamante». Abierto, no bloqueante, dueño equipo.

Tampoco resuelve **PA-163** (el `detalle` que no llega al registro), que es
independiente y más pequeño.

---

## 10. Criterio de cierre

PA-162 se cierra cuando se cumplan **las tres**, en este orden:

1. Las siete pruebas en verde, y las cinco perturbaciones vistas en rojo.
2. `cargo xtask tablero`, `manual`, `evidencia` y `verificar` en verde, con
   `sembrar` documentada en `docs/Comandos.md`.
3. **La calma de RUA con `condiciones=ninguna`** desde `base-inventario-sembrado`,
   leída en el colector.

El punto 3 no es ceremonial. Es el único de los tres que no puede pasar en verde
estando el mecanismo sin cablear — y es precisamente la clase de defecto que
estamos cerrando. Los puntos 1 y 2 habrían estado verdes toda la semana pasada.

---

## 11. Riesgos

- **El nombre `sembrar` no está acordado.** Se decide antes de escribir código.
- **La secuencia la lee `sembrar` del propio inventario.** Si `emitir` y
  `sembrar` discreparan en cómo la obtienen, la marca quedaría mal y el sensor
  rechazaría su propio inventario. La prueba 1 lo cazaría, pero conviene que las
  dos usen el mismo camino de lectura y no dos.
- **Un administrador puede olvidar `sembrar`.** El resultado es el estado de hoy:
  `inventarioNoVerifica` y manipulación. No es peor que ahora, pero es un sexto
  paso en una ceremonia manual, y los pasos manuales se olvidan. Queda anotado
  que la ceremonia debería acabar comprobándose a sí misma — un `eje-manifiesto
  revisar --almacen <dir>` que diga qué falta. Fuera de alcance aquí.
