# RPT-056 — Paridad de la vista: lo que no se compara, diverge

**Tema:** PA-106 y PA-102. Atar la línea del latido a `EMISIBLES`, y la vista al contrato
**Nº de reporte:** 056
**Fecha:** 13 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado.** Cierra PA-102 y PA-106

- **Depende de:** RPT-055 (la décima condición), RPT-053 (el latido), RPT-046 (el lector de fuentes)
- **Aborda:** PA-102, PA-106

---

## 1. PA-106 estaba mal enunciado, y el enunciado importaba

Se pidió «una prueba que valide que el IPC y syslog contienen la misma
información sobre las diez condiciones».

**No la contienen, y no deben.** `salidaNoDisponible` y `sinColector` no viajan
por syslog porque emitirlas exigiría el canal que falta (RPT-055 §4). Una prueba
de igualdad habría fallado por la razón correcta, y la salida obvia —hacerlas
emisibles— habría roto lo único que las hace útiles.

La paridad exigible son dos afirmaciones distintas, y cada una tiene su prueba:

1. **En el lado de syslog**: la línea del latido nombra exactamente lo emisible.
2. **En el lado de la vista**: VIS-04 nombra las diez del contrato.

Lo que las une no es que digan lo mismo, sino que **la diferencia esté
declarada**: `NO_EMISIBLES` la nombra, con motivo, en un solo sitio.

## 2. La superficie que nadie miraba

La barrera de PA-91 ata `EMISIBLES` a `Condiciones`. Lo que no ataba nada es que
**la línea del latido lleva su propia lista de nombres**.

Está construida iterando `EMISIBLES`, así que hoy coincide. Pero eso es una
propiedad de la implementación de hoy, no del contrato, y es exactamente el tipo
de coincidencia que sobrevive hasta que alguien optimiza el formato.

Si ahí faltara una condición, el técnico en sitio la vería por IPC y el operador
de sala no. Los dos creerían estar mirando el mismo sensor.

La prueba comprueba las dos direcciones —ni una más ni una menos— y que ninguna
de las dos no emisibles aparezca **en la línea entera**, no sólo en la lista: un
latido que las llevara en otro campo sería el mismo fallo.

## 3. PA-102: la vista es el sexto sitio

`vis04.js` y `diagnostico.js` no se compilan con `tsc`, no los cruza
`dependency-cruiser` y ninguna prueba los ejecutaba. Escriben los nombres de las
condiciones **a mano**, igual que `preload.cts` escribe los canales y por el
mismo motivo: no pueden importar del paquete base con la ventana en modo
estricto.

Cuatro pruebas nuevas:

- **Las diez, sin faltar ni sobrar.** Faltar es lo grave: una condición activa
  sin fila donde mostrarse desaparece sin que nada avise. Sobrar también importa:
  un nombre que el agente ya no manda se pintaría como `AUSENTE EN LA RESPUESTA`
  para siempre, y esa alarma permanente es cómo se enseña a ignorar la única
  señal que distingue *ausente* de *falso*.
- **El panel de diagnóstico traduce las diez.**
- **Todo identificador que `vis04.js` busca existe en su HTML.**
  `getElementById` devuelve `null` y el fallo aparece más tarde, en otra línea y
  como otra cosa.
- **El texto inicial del sello sigue declarando el estado roto** (PA-101). Era
  una decisión sin prueba: cualquiera podía «limpiarla» a un texto neutro y
  devolver la ventana al estado en que un módulo que no arranca es
  indistinguible de un tablero sin datos.

Todas leen el fuente **sin comentarios**, con el lexer de PA-73. Un comentario
que cite un nombre haría pasar la paridad con el nombre real borrado: falso
negativo silencioso en la única barrera que protege este sitio.

## 4. Dos cosas que la propia prueba enseñó

**`sinComentarios` salió de `preload.prueba.ts` a `lexico.ts`.** Importarlo desde
otro fichero de pruebas habría vuelto a ejecutar la suite del preload en el
segundo proceso: los mismos seis casos contados dos veces. Es la clase de cifra
que hace creer que la cobertura subió.

**Y al hacerlo apareció que el patrón de la suite era `**/*.js`**, así que
`lexico.js` —un fichero sin una sola aserción— salió listado como suite en verde.
Un fichero que pasa sin comprobar nada suma al recuento y no sujeta nada. El
patrón pasa a `**/*.prueba.js`.

No es cosmético: la cifra de la suite es de las pocas señales que este proyecto
usa para decidir si algo está cubierto, y una señal que sube sin motivo la
degrada entera.

## 5. Lo verificado

`cargo clippy --workspace --all-targets -- -D warnings` limpio. 92 pruebas en
`eje-agente`. En `eje-vision`, las cuatro comprobaciones de `verificar` en verde.

## 6. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-102~~ | ✅ La vista atada a sus identificadores y al contrato |
| ~~PA-106~~ | ✅ Paridad declarada, no igualdad supuesta |
| PA-103 | La rama `noServido` del panel sigue sin ejecutarse nunca |
| ~~PA-105~~ | ✅ `eje-vigia`, RPT-057 |
| ~~PA-104~~ | ✅ Cerrado por observación en RPT-057 §4 |

---

*Reporte Nº 56 — Paridad de la vista · PremosCorp · 13 de agosto de 2026*
