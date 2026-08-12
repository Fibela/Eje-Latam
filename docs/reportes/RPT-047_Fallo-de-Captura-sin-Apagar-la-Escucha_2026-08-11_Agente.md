# RPT-047 — Un fallo de captura no debe apagar la escucha

**Tema:** Qué hace el agente cuando no puede observar, y por qué seguir vivo no es obviamente lo correcto
**Nº de reporte:** 047
**Fecha:** 11 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Diseño. Pendiente de ratificación.**

- **Depende de:** RPT-034 (servicio continuo), RPT-036 (rechazo con motivo), RPT-046 (observación real)
- **Aborda:** PA-81

---

## 1. Lo que pasa hoy, observado

```text
Escucha local      : /tmp/eje/agente.sock
Error: Captura(PrivilegiosInsuficientes { interfaz: "lo" })
```

El socket se abrió. Un instante después el proceso murió y se lo llevó.

En ese mismo arranque el agente ya sabía otras dos cosas —«SIN CLAVE
aprovisionada» y «Requiere acción del administrador»— que son exactamente lo que
`accionAdministrativa` existe para enseñar en VIS-04. Las tenía, abrió el canal
para servirlas, y se murió antes de que nadie pudiera pedírselas.

**El momento en que más falta hace la consola es aquel en el que el agente no
está.**

## 2. La objeción, que es buena y hay que responderla

Un agente muerto lo nota el supervisor: `systemd` lo reinicia, la alerta salta,
alguien mira. Un agente **vivo que no captura** puede parecer sano: el proceso
existe, el socket responde, el panel pinta.

Cambiar «muere ruidosamente» por «vive en silencio» sería un empeoramiento, y
sería exactamente la clase de decisión que este proyecto lleva semanas evitando.

La propuesta sólo se sostiene si el agente degradado es **imposible de confundir
con uno sano**. De ahí que el §3 no sea un detalle de presentación: es la
condición que hace admisible el §4.

## 3. Una condición nueva, y por qué no vale la que hay

Existe `capturaConPerdida`: la captura funciona y se pierden tramas, la vista es
**incompleta**. Reutilizarla aquí sería decir que se ve mal cuando no se ve nada.

Es el error de RPT-036 §6 otra vez: «no hay nada» y «esto no lo estoy mirando»
no son lo mismo, y colapsarlas es cómo un operador concluye que en ese segmento
no pasó nada.

Se propone **`capturaNoDisponible`**, novena condición:

| Condición | Significa |
|---|---|
| `capturaConPerdida` | Observo, y se me escapan tramas |
| `capturaNoDisponible` | **No observo nada** |

### 3.1. Rectificación: el motivo no viaja en la condición

Este reporte decía en su primera redacción que el motivo —privilegios, interfaz
inexistente, interfaz desaparecida— debía viajar con la condición «no colapsado a
un booleano». Al mirar el código, no se sostiene.

`Condiciones` describe **lo que es verdad ahora mismo**, con una forma uniforme
que seis sitios comprueban. Un campo de texto que carece de sentido durante las
horas en que la condición es falsa no pertenece a esa forma, y romper la
uniformidad obligaría a un camino distinto en cada serializador.

El motivo no es un estado: es parte de un **suceso** —«dejó de capturar, por
X»—. Va donde van los sucesos y donde de hecho se diagnostica: el asiento de
ALM-01 que anota la transición, y la línea de syslog.

VIS-04 muestra que no se observa; el porqué está a un clic, en el registro.

El nombre final es **`capturaNoDisponible`** y no `capturaInactiva`: ya existe
`salidaNoDisponible` con el mismo sentido —el canal está y no responde— y usar
otro giro para el mismo tipo de estado invitaría a deducir una diferencia que no
hay.

Coste: el sexto sitio de siempre. `Condiciones` pasa a 9 campos en `eje-ipc`,
`contrato-ipc.toml`, `puente.ts`, `CAMPOS_CONDICIONES`, la salida syslog y
VIS-04. Las barreras de PA-20 y PA-75 obligan a tocarlos todos; ése es su
trabajo.

## 4. Qué hace el agente sin captura

1. **No termina.** `abrir` deja de propagarse con `?` en el arranque.
2. **Declara la condición** con su motivo, en cada vuelta.
3. **Emite por syslog**, con la gravedad de una amenaza no contenida. Un sensor
   que dejó de observar es un incidente operativo, no un aviso.
4. **Sirve consultas.** Es lo único que puede hacer y es precisamente lo que hace
   falta para diagnosticarlo.
5. **Reintenta abrir** en cada vuelta. Una interfaz que aparece tarde —arranque
   del sistema, cable reconectado— debe recuperarse sola. Al conseguirlo, la
   condición se apaga y queda constancia en ALM-01.

El punto 3 es el que responde al §2: el supervisor deja de ser la única señal.

## 5. Lo que NO se propone

**No se anexa una alerta por vuelta.** Es una condición, no un suceso: se es
verdadera hasta que alguien interviene, y anotarla dos veces por segundo
inundaría ALM-01 (RPT-019 §2). Lo que sí se anexa es la **transición** — dejó de
capturar, volvió a capturar—, que son dos asientos por episodio.

**No se finge inventario.** Sin captura no hay dispositivos vistos.
`obtener-inventario` no debe devolver una lista vacía como si el segmento
estuviera desierto; debe rechazar con motivo, igual que hace hoy con los canales
sin manejador.

**No se reintenta con espera creciente.** El ciclo ya marca ~500 ms y reintentar
en cada vuelta es barato. Añadir retroceso exponencial sería complejidad sin
problema medido, y `PA-83` ya dice que la cadencia no está medida.

## 6. Lo que hace falta decidir y no decido yo

**Si un sensor sin captura debe seguir arrancando o no.** Aquí se propone que sí,
por lo del §1. Pero en un despliegue donde el agente se instala desatendido, un
arranque que «funciona» sin observar nada puede darse por bueno durante meses.

La alternativa sería arrancar, servir, y **terminar tras N vueltas sin
recuperarse**, de modo que el supervisor acabe enterándose igual pero después de
haber dado tiempo a preguntar. No lo propongo porque N sería otro número sin
medir, pero la pregunta es de producto y conviene contestarla a propósito.

## 7. Lo que propongo ratificar

1. `capturaNoDisponible` como novena condición, **con motivo**, distinta de
   `capturaConPerdida` (§3).
2. El comportamiento del §4, con la emisión por syslog como parte no opcional.
3. Las tres exclusiones del §5.
4. Dejar el §6 abierto y decidido por producto, no deducido del código.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| **PA-87** | ¿Debe un sensor que no observa terminar tras N vueltas? (§6) |
| PA-81 | Se cierra al implementar esto |

---

*Reporte Nº 47 — Fallo de captura sin apagar la escucha · PremosCorp · 11 de agosto de 2026*
