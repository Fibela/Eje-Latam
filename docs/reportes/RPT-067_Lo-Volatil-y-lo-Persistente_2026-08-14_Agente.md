# RPT-067 — Lo volátil y lo persistente

**Tema:** PA-120. El socket sale del directorio de evidencia
**Nº de reporte:** 067
**Fecha:** 14 de agosto de 2026
**Área designada:** Agente
**Estado:** Construido y probado en unidad. **Sin observar en sistema instalado** — eso es PA-117
**Entidad:** PremosCorp

- **Depende de:** RPT-035 (el socket local), RPT-062 (la unidad), RPT-065 §9.3 (la pregunta), RPT-002 §9.3 (nada de puertos TCP locales)
- **Aborda:** PA-120

---

## 1. Lo que **no** se encontró

Conviene abrir por aquí, porque el registro histórico se falsifica solo si nadie
lo vigila.

No se observó ningún fallo. No se midió que los permisos del directorio de datos
permitieran a nadie leer evidencia por estar el socket allí. Lo que hubo fue una
**disposición advertida al escribir el manual** (RPT-065 §9.3) y no medida.

Notar una disposición no es medir un defecto. Este cambio se justifica por otras
tres razones, todas comprobables sin haber medido nada.

## 2. Las tres razones que sí se sostienen

**El estándar.** `/run` es el sitio de los datos volátiles de un servicio y
`/var/lib` el de los persistentes. No es burocracia: son ciclos de vida
distintos, y juntarlos obliga a que un mismo permiso autorice dos cosas.

**El socket huérfano deja de ser posible.** `/run` es `tmpfs` —se vacía en cada
arranque— y `RuntimeDirectory=eje-latam` hace que `systemd` cree el directorio al
arrancar y lo **destruya al parar**. El fallo (4) de la puesta en marcha local
—un fichero que sobrevive al proceso y hace que el cliente reciba
`ECONNREFUSED` sobre algo que existe— se erradica por construcción, no por
disciplina.

**La medición de PA-117 pasa a ser de una sola cosa.** Es la razón operativa, y
la de más peso ahora mismo. Ver §3.

## 3. Por qué era prerrequisito de PA-117

La unidad decía, y sigue diciendo:

```ini
ReadWritePaths=/var/lib/eje-latam
```

Mientras el socket vivió ahí, esa línea autorizaba **a la vez** escribir
evidencia y crear el socket. Una prueba de `ProtectSystem=strict` sobre ese
montaje habría medido un directorio con dos cometidos, y un verde no habría
distinguido cuál de los dos lo justificaba.

Separados, la afirmación que PA-117 va a comprobar es exacta: el agente puede
escribir su registro y **nada más**.

## 4. Lo que se puede mover y lo que no

```rust
pub const DIRECTORIO_SOCKET_POR_OMISION: &str = "/run/eje-latam";
const NOMBRE_SOCKET: &str = "agente.sock";
```

El **directorio** es configurable con `--directorio-socket`. El **nombre del
fichero** no lo es, y esa asimetría es deliberada: si la ruta completa fuera
configurable, nada impediría apuntarla de vuelta a `/var/lib/eje-latam` y
deshacer esta separación sin que ninguna comprobación se enterase.

`--directorio-socket` existe por una sola razón, y conviene que quede escrita:
crear `/run/eje-latam` exige root, y obligar a `sudo` para levantar la consola de
diagnóstico haría que nadie la levantara. El guion de desarrollo apunta al
almacén de pruebas; la unidad **no pasa el argumento**, para que el valor de
fábrica sea el único sitio donde cambia.

## 5. La coincidencia que ahora es una prueba

`RuntimeDirectory=eje-latam` significa `/run/eje-latam`. Esa regla es de
`systemd`, no nuestra, y el valor de fábrica del agente tenía que coincidir con
ella **de memoria**.

Si alguien cambiara uno de los dos, el servicio arrancaría, no encontraría el
directorio, y la consola se quedaría sin nadie al otro lado. No fallaría el
arranque: fallaría lo que el arranque sirve.

Ahora hay una prueba que compone la regla y compara:

```rust
assert_eq!(
    format!("/run/{declarado}"),
    guardian_cc::arranque::DIRECTORIO_SOCKET_POR_OMISION,
);
```

Con dos más al lado: que la unidad **no** repita el argumento —o la comparación
de arriba dejaría de significar nada mientras un tercer valor manda— y que
`ReadWritePaths` cubra la persistencia y nada más.

## 6. Lo que el instalador no tuvo que cambiar

Nada. Ya creaba sólo `DESTINO_BIN`, `DESTINO_CONF`, `DESTINO_DATOS` y
`DESTINO_UNIDAD`, y ninguno es el directorio del socket. El ciclo de vida
volátil queda entero en manos de `systemd`, que es lo que se quería.

Queda anotado porque el impulso era añadir un `install -d /run/eje-latam` «por si
acaso», y eso habría creado un directorio persistente en el sitio de los
volátiles, que sobreviviría a la parada del servicio y devolvería exactamente el
problema que este reporte cierra.

## 7. La prueba se acusó a sí misma

La comprobación de que la unidad no pasa `--directorio-socket` se escribió así:

```rust
assert!(!unidad_de_servicio().contains("--directorio-socket"));
```

Y **falló**. No porque `ExecStart` pasara el argumento —nunca lo pasó— sino
porque el comentario que explica que no se pasa contiene esas mismas letras. La
prueba miraba el fichero entero y se acusó a sí misma.

Es la cuarta vez que aparece la misma familia: **toda comprobación que lee texto
tiene que quedarse antes con la parte que decide**. Ocurrió con los comentarios
de Rust en el guardián, otra vez en TypeScript, otra en la cobertura, y ahora con
la prosa de un fichero `.service`. La comprobación ahora extrae la orden de
arranque —`ExecStart` y sus continuaciones— y mira sólo ahí.

**Y lleva una guarda**, que es lo que impide que el arreglo sea peor que el
fallo:

```rust
assert!(arranque.contains("--interfaz"), "la orden de arranque no se extrajo bien");
```

Sin ella, un extractor que devolviera cadena vacía haría pasar la afirmación
negativa siempre. Una prueba que no puede fallar es indistinguible de no tener
prueba, y este era el momento exacto de fabricar una.

**La lectura del fallo por parte del equipo fue la equivocada**, y merece quedar
escrito: se diagnosticó que `ExecStart` inyectaba el argumento y se propuso
retirarlo de ahí. Seguir esa instrucción habría borrado el comentario —la única
línea que dice el porqué— y la prueba se habría puesto verde sin comprobar nada.
El montaje que produce el fallo y el que se supone que lo explica no son el
mismo, que es la lección de RPT-064 §6 aplicada a un mensaje de `cargo test`.

## 8. Lo que este reporte **no** puede afirmar

Que funcione en una máquina de verdad. Todo lo de arriba está probado en unidad y
en el texto de la unidad; nadie ha arrancado el servicio y visto aparecer
`/run/eje-latam`, ni lo ha parado y visto desaparecer.

Por eso PA-120 queda **parcial** y no cerrado. Lo cierra la misma observación que
cierra PA-117, y los comandos están escritos en `docs/Comandos.md` §9.3 antes de
ejecutarlos.

Nótese además que **nada de lo de §7 se descubrió mirando el código**: se
descubrió porque una prueba falló. Las tres comprobaciones nuevas de la unidad
son texto sobre texto, y ninguna dice si el servicio arranca.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| PA-120 | 🔵 Parcial: construido y probado en unidad; falta verlo en un sistema instalado |
| PA-117 | Cierra los dos: `kill -9` + `ProtectSystem=strict`, ahora sobre un `ReadWritePaths` sin ambigüedad |
| PA-79 | `--directorio-socket` es un argumento de línea de órdenes, y debería salir de configuración firmada |

---

*Reporte Nº 67 — Lo volátil y lo persistente · PremosCorp · 14 de agosto de 2026*
