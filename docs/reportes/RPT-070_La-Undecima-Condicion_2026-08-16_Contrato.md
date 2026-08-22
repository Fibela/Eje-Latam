# RPT-070 — La undécima condición

**Tema:** PA-125. Un sensor vivo e inalcanzable se declaraba sano
**Nº de reporte:** 070
**Fecha:** 16 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Cerrado por observación en máquina real.** Cierra PA-125

- **Depende de:** RPT-069 §4 (el hallazgo), RPT-055 (la décima), RPT-047 (el sensor ciego), RPT-006 §4
- **Aborda:** PA-125

---

## 1. No se dedujo: se vio ocurrir

Mientras el sensor de la máquina de pruebas arrancaba **sin escucha local** —la
consola no podía conectarse a él por ningún medio— las diez condiciones decían
esto:

```
capturaNoDisponible   : false
accionAdministrativa  : true
salidaNoDisponible    : false
sinColector           : true
```

Todo correcto. Ninguna decía lo único que importaba.

Es la misma forma que PA-109, con una diferencia que conviene anotar: **aquella
se descubrió razonando sobre el contrato y ésta se descubrió pasando**. Un
razonamiento encuentra los huecos que uno sabe buscar.

## 2. Por qué un sensor inalcanzable es peor que uno caído

Al caído lo reinicia el supervisor y alguien se entera. Éste **funciona**:
observa, clasifica, registra en ALM-01 y emite al SIEM. Sólo que nadie puede
preguntarle nada.

Un técnico que va a la planta porque un sensor no aparece en su consola no tiene
forma de distinguir «el equipo está apagado», «la red no llega» y «el agente está
perfectamente y su socket no existe». Las tres se ven igual: silencio.

Es el argumento de `capturaNoDisponible` (RPT-047 §2) trasladado del ojo al
puente.

## 3. La decisión que gobierna el cambio: **ésta sí se emite**

`salidaNoDisponible` y `sinColector` no viajan por syslog porque describen **el
canal de syslog mismo**: contarlas exigiría el canal que falla.

`escuchaNoDisponible` describe **el otro canal**. Cuando la escucha local cae,
syslog es justamente lo que sigue funcionando.

Y no es sólo que pueda: es que **es el único camino posible**. Lo que podría
contar que la consola no conecta es la consola, que es lo que no conecta. Si esta
condición no viajara, un sensor vivo e inalcanzable no existiría para nadie.

`EMISIBLES` pasa de 8 de 10 a **9 de 11**, y la barrera de PA-91 —que exige
`EMISIBLES.len() == todas - NO_EMISIBLES.len()`— lo comprueba sin tocarla.

## 4. Dónde vive cada mitad

| Pieza | Qué hace |
|---|---|
| `Condiciones::escucha_no_disponible` | El campo, con `enumerar()` y `CAMPOS_CONDICIONES` a 11 |
| `contrato-ipc.toml` | La autoridad. Declara el campo y por qué **sí** se emite |
| `condiciones(…)` | Lo recibe como **parámetro**, no lo rellena después |
| `Ciclo::declarar_escucha` | El interruptor, gemelo de `declarar_captura` |
| `main.rs` | Lo declara **en cada vuelta**, aunque la escucha se abra una sola vez |
| `EMISIBLES` | Con gravedad alta y sin acusar a nadie |
| `puente.ts`, `vis04.js`, `diagnostico.js`, `cabecera.ts` | Los cuatro sitios de la vista |

**Parámetro y no campo rellenado después**, por lo mismo que en PA-81: un campo
que se fija más tarde es un campo que alguien olvidará fijar, y el olvido se lee
como «la consola puede conectarse».

**Declarado en cada vuelta** aunque la escucha se abra una sola vez (PA-66): un
estado que sólo se fija al degradarse se queda pegado si alguien olvida el camino
de vuelta. Hay prueba de eso.

## 5. La rama que casi se queda sin prueba

`cabecera.ts` ganó una rama y la suite entera pasó en verde sin ejercitarla. La
tabla de la vista sí estaba cubierta —`vista.prueba.ts` compara contra el
contrato— pero **el mensaje que leería el técnico no lo comprobaba nadie**.

Es la clase de defecto dominante de este proyecto, la número doce o trece, y esta
vez apareció por mirar el diff en lugar de por moverlo. Se añadieron dos pruebas.

## 6. Por qué VIS-04 casi nunca la verá encendida, y aun así tiene fila

Para leer el panel hay que estar conectado, y si la condición está activa no se
puede estar. Llega en dos casos:

- una consulta que alcanzó al agente **justo antes** de que la escucha cayera;
- un **segundo agente en la misma máquina** cuyo socket sí responde — el
  despliegue de RPT-059, un agente por segmento en un servidor perimetral.

Callarla en la vista habría dejado al técnico con un panel que no explica por qué
el otro sensor no aparece.

## 7. La observación que lo cierra

Agente real, en la máquina virtual, con `--directorio-socket /run/no-existe` —un
directorio inexistente hace fallar la apertura del socket **también siendo
root**, sin tocar permisos de nada— y una sala falsa de doce líneas escuchando el
5514.

En su propio informe, once condiciones:

```
    accionAdministrativa  : true
    salidaNoDisponible    : false
    sinColector           : false
    escuchaNoDisponible   : true
```

Y en la sala:

```
<107>1 … eje-agente - condicion - condicion=escuchaNoDisponible estado=activa
<110>1 … eje-agente - latido-de-sensor - latido=1 … condiciones=accionAdministrativa,escuchaNoDisponible
```

Un sensor al que ninguna consola puede llegar ha dejado de presentarse sano.

**El `<107>` no se buscaba y confirma el diseño.** `accionAdministrativa` llega
como `<108>` —aviso— y ésta un escalón por encima, con la gravedad de la
manipulación sin serlo. Es la convención de `registroSaturado`: el segundo campo
de `EMISIBLES` no dice «esto es un ataque», dice «esto no puede esperar al
lunes». Y `hay_manipulacion()` sigue sin incluirla, así que VIS-04 no la
presentará como que alguien tocó nada.

## 8. Tres montajes fallidos antes de medir nada

Se anotan porque los tres produjeron salidas que **parecían resultados**:

- **La sala falsa aceptaba una sola conexión.** El agente abre una por envío, así
  que capturó la primera transición y colgó; los ciclos siguientes mostraban
  `salidaNoDisponible: true`, que era la respuesta correcta a un colector que yo
  había escrito mal.
- **`/tmp/sala.py` desapareció entre sesiones**, porque `/tmp` se vacía al
  arrancar la máquina. Mismo síntoma, otra causa.
- **El binario de la VM era el del día anterior**, tres ejecuciones seguidas. La
  lista salía con diez condiciones y la conclusión inmediata habría sido que el
  campo no funcionaba.

Lo tercero se cazó por una regla de forma, no por sospecha: `presentar` **itera**
`enumerar()`, así que un binario con once campos no puede imprimir diez. La
comprobación decisiva —`md5sum` a los dos lados— tardó veinte segundos y se
saltó dos veces antes de hacerse.

**Copiar un artefacto y comprobar que llegó no son el mismo acto.** El `md5sum`
pasa a ser parte del protocolo de RPT-068, no un paso opcional.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-125~~ | ✅ **Cerrado por observación** (§7) |
| PA-123 | El informe completo cada vuelta, a `journald` |
| PA-122 | La línea de uso y el analizador, dos listas a mano |
| PA-126 | El formato de distribución del paquete |

---

*Reporte Nº 70 — La undécima condición · PremosCorp · 16 de agosto de 2026*
