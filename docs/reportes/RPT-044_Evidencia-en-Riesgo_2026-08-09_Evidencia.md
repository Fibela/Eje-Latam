# RPT-044 — La evidencia que no llegó al disco

**Tema:** Que una alerta anexada y no escrita deje de ser un mensaje de consola
**Nº de reporte:** 044
**Fecha:** 9 de agosto de 2026
**Área designada:** Evidencia
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-69

- **Depende de:** RPT-032 (sin colas), RPT-037 (el ciclo), RPT-038 (testigo), RPT-042 y RPT-043 (paridad de uso)
- **Cierra:** PA-69

---

## 1. Lo que faltaba no era dónde guardarla

La alerta no estaba perdida: estaba en el registro en memoria, que **es** su sitio.
Lo que no estaba era la constancia de que no había llegado al disco: `main.rs` lo
imprimía y VIS-04 no tenía por dónde enterarse.

Por eso no hay búfer de contingencia. RPT-032 §5 ya rechazó una cola para la
salida por syslog —«una cola de alertas no enviadas que crece sin límite es el
agotamiento de memoria de RPT-018 §6 con otro nombre»— y aquí el argumento es
idéntico y además peor: con el disco lleno, un búfer que crece termina matando el
proceso, y entonces se pierde **todo** lo acumulado.

## 2. El defecto de fondo estaba en la guarda, no en el aviso

```rust
if !anexadas.is_empty() {
    match persistir(&self.evidencia, &self.registro) { ... }
}
```

Sólo se escribía **si esa vuelta anexó algo**. Una vuelta anexaba, la escritura
fallaba, las siguientes no anexaban nada —lo normal en un sensor tranquilo— y
**nadie volvía a intentarlo**. El disco se recuperaba a los diez segundos y el
agente seguía con las alertas sólo en memoria hasta la amenaza siguiente; si el
proceso moría antes, se iban todas.

La guarda pasa a ser «anexó **o** lo de antes no llegó». Ahí vivía la pérdida
real, y no estaba en el enunciado del punto.

## 3. El asiento se anexa al recuperar, no al fallar

Anexarlo al fallar es circular: el asiento que dice «la escritura falló» iría al
registro que no se puede escribir, y moriría con el proceso igual que las alertas
que pretende explicar. Y con el disco lleno, añadir bytes empeora el intento
siguiente.

Al recuperar, el disco funciona **por definición**. El asiento
`persistencia-restablecida` describe el tramo entero —cuántas vueltas, qué estuvo
sólo en memoria— y se escribe con una segunda persistencia, para que la constancia
misma sea durable.

## 4. Dos mecanismos porque uno solo no basta

**La condición `evidenciaEnRiesgo`** (octava) da la visibilidad en vivo por IPC. Se
apaga sola cuando el disco vuelve, y por eso **no basta**: un fallo de dos segundos
puede no aparecer en ninguna consulta.

**El asiento** da la constancia duradera. Queda en la cadena, firmado por el
encadenamiento, y un auditor puede leerlo dentro de dos años.

## 5. El caso que no se cierra en local, dicho aquí

Si el proceso muere **durante** el fallo, no hay constancia posible en el disco:
las alertas y el asiento que las explicaría estaban ambos en memoria.

Lo único que queda es externo. El sello de PA-64 no se emite cuando lo anexado no
es durable, así que el colector ve que **el extremo dejó de avanzar** y después un
salto. Es evidencia en el SIEM, no nuestra, y depende de que haya colector.

Se escribe aquí para que nadie lea PA-69 como «la evidencia ya no se pierde
nunca». Se pierde menos, se avisa siempre que se pueda, y hay un caso en que la
única prueba está fuera del equipo.

## 6. La lógica salió de `vuelta` para poder probarla

`asegurar_durabilidad` es una función aparte por el motivo de PA-68: dentro del
ciclo no se puede ejercitar, porque `PrimerArranque` nunca produce un veredicto
prohibido y las pruebas de `eje-agente` no pueden construir un inventario firmado.

Las tres pruebas usan un **fallo de disco real**: ocupan la ruta del registro con
un directorio, así que ningún renombrado atómico puede caer encima. El código ve
el mismo error que vería con el volumen lleno o de sólo lectura. Nada de dobles.

## 7. Las dos pruebas que escribí y tiré

Las primeras dos versiones eran flojas. Una de ellas afirmaba
`assert_eq!(x, { let mut y = x; y.campo = false; y })`, que es trivialmente cierto
y no comprueba nada; la otra terminaba verificando que `persistir` falla sobre un
directorio, sin llegar a ejercitar el reintento.

Las descarté antes de entregarlas, pero queda escrito: es exactamente el patrón
que llevamos ocho reportes persiguiendo —el mecanismo que existe y no comprueba
nada— cometido en la prueba hecha para cazarlo. Esta clase de cosa sólo deja de
repetirse si se anota.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| ~~PA-69~~ | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 44 — La evidencia que no llegó al disco · PremosCorp · 9 de agosto de 2026*
