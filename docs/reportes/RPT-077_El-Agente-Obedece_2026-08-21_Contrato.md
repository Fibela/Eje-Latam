# RPT-077 — El agente obedece

**Tema:** PA-79 paso 4b. La configuración firmada deja de ser una declaración y pasa a mandar
**Nº de reporte:** 077
**Fecha:** 21 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** Construido y probado. Cierra el paso 4 de RPT-074 §10

- **Depende de:** RPT-074 (el formato y los tres estados), RPT-067 §3 (el socket fuera del almacén), RPT-054 §7 (`Restart=always`), RPT-072 (el modo que se calla)
- **Aborda:** PA-79

---

## 1. Lo que faltaba, dicho sin adorno

Después del paso 4a el agente imprimía **«Configuración: firmada y verificada»** y
a continuación vigilaba la interfaz que le decía el `ExecStart`.

No era un paso a medias por descuido: se dejó así a propósito para que las dos
condiciones nuevas fueran verdaderas desde el primer commit. Pero mientras
existieran las dos vías, la firma **no valía nada**. Una línea en la unidad le
gana a una firma ML-DSA-65.

## 2. Los tres caminos

| Estado | Quién manda | Qué hace el sensor |
|---|---|---|
| **Firmada** | La configuración, entera | Vigila lo que dice. Una bandera dictada en la línea de órdenes **aborta el arranque** |
| **Ausente** | La línea de órdenes | Vigila y lo declara con `configuracionSinFirmar`. Es como corre toda la flota hoy |
| **No verifica** | **Nadie** | Arranca, no vigila nada, no emite nada, y dice por qué |

## 3. Por qué se rechaza incluso el argumento que coincide

`--interfaz eth0` con `interfaz = "eth0"` firmado también impide arrancar.

Comparar obligaría a decidir qué significa «igual» para cada tipo: una ruta con
barra final, un colector con mayúsculas, un intervalo escrito de otra forma. Cada
una de esas decisiones es un sitio donde colar un valor que **pasa por igual sin
serlo**. La regla sin grados no tiene ese sitio:

> Con configuración firmada, la línea de órdenes no dicta nada.

## 4. El círculo que apareció al cablearlo

El formato de RPT-074 firmaba también `almacen` y `directorio_socket`. Al escribir
el orden de arranque salió esto:

```
para verificar la configuración hace falta <almacen>/clave-cliente.pub
para saber dónde está el almacén hace falta leer la configuración
```

No es un problema de orden de líneas. **Una configuración que decide dónde está el
almacén es una configuración que decide dónde se busca la clave que decide si
creerla.** Basta apuntarla a un directorio propio, dejar ahí una clave propia, y la
firma pasa a avalar lo que uno quiera.

Los dos campos salieron del formato — de seis campos de texto a cuatro. La
distinción que faltaba:

- **política** — a qué segmento mira, cada cuánto late, a quién informa, quién
  puede consultarlo, cómo se llama en la sala. La firma el cliente.
- **instalación** — dónde guarda sus ficheros esta máquina. Lo decide quien la
  instala, y vive en la unidad.

Mezclarlas fue lo que produjo el círculo. Y no se sujeta con un comentario: el
almacén **no está en `Efectivas`**, se fija antes en `rutas_de_instalacion`, y así
la resolución no puede moverlo aunque alguien lo intente.

## 5. La decisión que cambié después de aprobada

Lo aprobado era: **firma rota → el agente no arranca.** Al construirlo apareció que
eso deja huérfana `configuracionNoVerifica`, una condición estrenada ayer. Si el
agente muere, nunca la enciende nadie; sería un mecanismo sin cablear recién hecho,
que es el defecto dominante de este proyecto cometido al cerrarlo.

Y hay un segundo motivo, operativo: con `Restart=always` (RPT-054 §7) un agente que
sale con error es un **bucle de reinicios**, no una avería visible. Para la sala un
sensor muerto es indistinguible de un cable cortado.

Así que el tercer caso arranca — pero **no obedece a nadie**:

- sin interfaz: no captura, y `capturaNoDisponible` lo dice;
- sin colector: no emite, y `sinColector` lo dice;
- sin grupo: el socket queda en `0600`;
- **y sin caer a la línea de órdenes**, que es lo que sujeta la seguridad. A quien
  pudo tocar el fichero le bastaría romperlo para recuperar el mando por argumentos.

Es la versión del **estado degradado declarado** que este proyecto ya eligió sobre
la estricta. Vivo y declarando es un diagnóstico; muerto no es nada.

## 6. La unidad ya no configura el sensor

`EnvironmentFile` fuera. `ExecStart` se queda con lo que no es política:

```ini
ExecStart=/usr/local/bin/eje-agente \
    --almacen /var/lib/eje-latam \
    --ciclos 0
```

**Un sensor recién instalado no arranca vigilando nada hasta que se le emite
configuración.** Es un corte deliberado, y el instalador lo grita con los tres
pasos y con el `hostname` que hay que poner. La alternativa era dejar en pie el
camino que la firma viene a cerrar.

RPT-054 §4.1 se ratificó como «instala y lo declara a gritos». El grito cambia de
asunto —antes el colector ausente, ahora la configuración ausente, que es más
grande y lo contiene— pero la mitad que ve la persona delante de la máquina sigue
existiendo, y la caja de arena la sigue midiendo.

## 7. Dos barreras que no se pueden quedar cortas

El defecto de la semana son los índices escritos a mano. Aquí había dos sitios
donde volvería a pasar, y ninguno es una lista:

**Las pruebas recorren `OPCIONES`.** Una bandera nueva marcada `dictada` queda
cubierta el día que se añade; una marcada `dictada: false` también, por la mitad
contraria — sin ella, marcarlo todo como dictado pasaría la primera prueba y
dejaría una unidad que no puede arrancar.

**La correspondencia con el formato se sujeta desestructurando `Valores`.** Añadir
un campo a la configuración firmada **no compila** hasta que alguien decida si es
un parámetro del sensor —y le ponga su bandera— o una defensa como la secuencia.
Sin eso, un campo nuevo sería configurable por la línea de órdenes y nadie se
enteraría.

**Y la plantilla que viaja en el paquete la analiza el emisor de verdad**, no un
`contains`. Un campo mal escrito en ella fallaría en la planta, delante del
técnico, el día de la instalación.

## 8. Lo que sigue sin hacer

El paso 5: atar la `secuencia` al centinela. Hoy el formato la lleva y el emisor la
incrementa leyendo la anterior, pero **el agente no la compara con nada**: una
configuración antigua y correctamente firmada se acepta. La defensa contra
reversión está escrita y no está conectada — que es, otra vez, la familia de
siempre.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| PA-79 | 🔵 Sigue parcial: hecho el paso 4 entero, falta el 5 (§8) |
| PA-14a | Sin cambios. La firma de release sigue bloqueando la autenticidad del paquete |

---

*Reporte Nº 77 — El agente obedece · PremosCorp · 21 de agosto de 2026*
