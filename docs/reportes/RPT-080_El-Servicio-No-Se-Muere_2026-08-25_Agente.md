# RPT-080 — El servicio no se muere

**Tema:** PA-133. Un sensor sin aprovisionar entraba en bucle de reinicios en vez de declararlo
**Nº de reporte:** 080
**Fecha:** 25 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** Construido y probado. Cierra PA-133

- **Depende de:** RPT-077 §5 (el argumento, escrito y no aplicado), RPT-072 (`--ciclos` como frontera entre servicio y mano), RPT-054 §7 (`Restart=always`), RPT-079 §11 (la observación)
- **Aborda:** PA-133

---

## 1. Trescientas cincuenta veces

```
eje-agente.service: Scheduled restart job, restart counter is at 350.
eje-agente.service: Failed with result 'resources'.
```

Eso estaba en el diario de la VM cuando fuimos a probar otra cosa. El servicio
llevaba media hora arrancando y muriendo cada cinco segundos, y **nada fuera de
ese diario se enteró**: sin proceso no hay socket, sin socket no hay consola, y sin
colector no hay latido. Para la sala, un sensor que nunca late es indistinguible
de uno que aún no se ha instalado.

## 2. El defecto no es una rama mal escrita

RPT-077 §5 argumentó, para la configuración **rota**:

> *Un agente que se cae con `Restart=always` es un bucle de reinicios, y para la
> sala un sensor muerto es indistinguible de un cable cortado. Vivo y declarando
> es un diagnóstico.*

Y no se aplicó a la configuración **ausente**, que es el estado en el que llega
**toda máquina nueva**. La unidad dejó de pasar `--interfaz` —lo dicta la firma—,
así que sin firma el agente no sabía qué mirar y devolvía error de uso.

**Una regla que se aplica caso por caso se olvida en el siguiente caso.** De ahí
que el arreglo no sea corregir la rama sino fijar la regla (§5).

## 3. La prosa iba por delante del mecanismo

Lo que más incomoda de este punto: el comportamiento correcto **ya estaba escrito
en dos sitios**, desde el mismo día.

El instalador decía, y sigue diciendo:

> *Arrancarlo ahora no da un sensor a medias: da uno que lo declara y espera.*

Y RPT-077 §6 decía «no arranca» donde debía decir «no vigila».

Las dos frases describen la intención. El código hacía otra cosa. Es la familia de
siempre —lo declarado y lo cableado divergen— **con los papeles cambiados**: aquí
la documentación tenía razón y el mecanismo no la seguía. No hay barrera que cace
eso leyendo texto, porque el texto era correcto.

Lo cazó arrancar el servicio en una máquina.

## 4. Quién pregunta decide la respuesta

Sin interfaz y sin configuración firmada hay dos situaciones distintas, y ya
existía una opción que las separa:

| Quién | `--ciclos` | Qué hace |
|---|---|---|
| El servicio | `0` | **Arranca y lo declara.** Morir es un bucle de reinicios |
| Una persona | `N` finito | **Explica el uso.** Hay alguien delante esperando saber qué falta |

No se añade una bandera nueva. RPT-072 ya usó `--ciclos` para decidir cuánto habla
el agente con el mismo argumento: *«ya existe una opción que dice exactamente eso,
y dos formas de decir lo mismo se contradicen el día que alguien cambie una»*.

Lo que sí se hace es **darle nombre**: `es_servicio(ciclos)`. De ella cuelgan ahora
las dos decisiones —cuánto habla y si morir—, en lugar de que cada una compare
`ciclos == 0` por su cuenta. Las dos preguntan lo mismo: *¿hay una persona
delante?*

## 5. La barrera, que es lo que de verdad se arregla

```rust
fn el_servicio_arranca_diga_lo_que_diga_la_configuracion()
```

Recorre los tres estados de `Configuracion` y exige que **ninguno** impida
arrancar en modo servicio. Las dos pruebas de los casos concretos son sus
ejemplos; ésta es la regla.

Y el `match` que enumera los estados es **exhaustivo a propósito**: añadir una
variante a `Configuracion` no compila hasta que alguien decida si el servicio
sigue arrancando con ella. Es el mismo recurso que sujeta los campos de `Valores`
en RPT-078 §6 — que el compilador obligue a decidir, en lugar de confiar en que
alguien recuerde.

## 6. Lo que este arreglo NO ablanda

Un sensor sin configuración firmada **sigue sin vigilar nada**. No se inventa una
interfaz por omisión, no se cae a un valor plausible, y no se disfraza de sensor
sano:

- `configuracionSinFirmar` encendida, y viaja al colector;
- `capturaNoDisponible` encendida, porque es verdad;
- y en la primera pantalla, con todas las letras:

```
Configuracion      : SIN FIRMAR, y no hay ninguna interfaz que vigilar
  !! ESTE SENSOR NO ESTA VIGILANDO NADA.
     Arranca, atiende consultas y lo declara. No observa.
     Emitele configuracion firmada: eje-manifiesto configurar
     El campo 'maquina' tiene que ser: <hostname>
```

La diferencia con antes no es que haga más. Es que **está vivo para poder decirlo**,
y que la consola puede preguntárselo por el socket.

Y de paso deja alcanzable `configuracionSinFirmar`, que desde RPT-077 no podía
encenderse en ningún despliegue real: un mecanismo sin cablear estrenado hacía
cuatro días.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-133~~ | ✅ **Cerrado** (§4, §5) |
| PA-135 | Sin cambios. Cuatro de los seis canales siguen declarados y sin cablear |

---

*Reporte Nº 80 — El servicio no se muere · PremosCorp · 25 de agosto de 2026*
