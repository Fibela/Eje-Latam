# RPT-054 — Empaquetado dual: qué se instala, y qué declara al instalarse

**Tema:** PA-107. Especificación del artefacto headless y del de diagnóstico, bajo «instalación abierta con declaración de estado»
**Nº de reporte:** 054
**Fecha:** 13 de agosto de 2026
**Área designada:** Producto
**Entidad:** PremosCorp
**Estado:** **Especificación. Pendiente de ratificación.**

- **Depende de:** RPT-051 (opción D), RPT-052 y RPT-053 (el latido), RPT-023 y RPT-025 (qué no se empaqueta), RPT-002 §9.3 (transporte)
- **Aborda:** PA-107. Condiciona PA-12, PA-65, PA-79
- **Nota de identificador:** este punto se llamó **PA-84** en RPT-051 y RPT-052. PA-84 estaba ya tomado en la guía de puesta en marcha. Ver RPT-053 §8

---

## 1. La decisión que gobierna el resto

**El artefacto headless se instala aunque no haya colector, y lo declara a
gritos.** Ratificado hoy.

El motivo es operativo antes que técnico: un técnico que llega al armario de
planta y no alcanza el colector por un error de red del cliente —ajeno al
agente— debe poder dejar el sensor funcionando en modo local. Bloquear la
instalación le obliga a volver a la sala, reconfigurar y volver al sitio. Y el
perfil que compra Local-First es exactamente el que puede no tener SIEM.

El precio se acepta a sabiendas: **se puede desplegar una flota entera sin
vigilar** si nadie lee lo que el agente declara. Todo el §4 existe para que eso
sea difícil.

## 2. Los dos artefactos

### 2.1. Sensor headless — el que se instala en el cliente

Un binario: `eje-agente`. Sin bibliotecas gráficas, sin Electron, sin NSS ni
CUPS ni ALSA ni GBM. RPT-046 midió lo que cuesta la ventana y por eso no va aquí.

Lleva: el binario, la unidad de servicio, la configuración, el directorio de
evidencia y la guía de puesta en marcha.

### 2.2. Consola de diagnóstico — la que se instala aparte

VIS-04 sobre Electron, **instalable por separado y no por omisión**. Es para el
técnico que va a la planta a averiguar por qué un sensor no captura.

No es la consola de operación: ésa lee del colector (RPT-051 §2C) y es otro
producto, con otro suministro de datos y otra pregunta pendiente (PA-106).

### 2.3. Lo que hace que esto no cueste el doble

La capa base —máquina de estados, cabecera, sucesos, bitácora, traducción de
fallos— es lógica pura y no depende del transporte. Está probada sin ventana,
sin agente y sin escritorio (RPT-051 §5). Los dos artefactos comparten esa capa;
lo que se duplica es la **verificación de los caminos**, no el código.

## 3. Lo que NO entra, y por qué una prueba de código no basta

**`eje-manifiesto` se queda fuera.** Es la decisión que sostiene los cinco
eslabones de RPT-011: si el emisor viviera en el binario del agente, cada sensor
desplegado llevaría encima **la capacidad de firmar inventarios**, y un sensor
está en un armario físicamente accesible cuyo modelo de amenaza asume que puede
caer.

Hoy hay una prueba que comprueba que `eje-agente` no declara `eje-manifiesto`
como dependencia. Es necesaria y **no es suficiente**: nada impide que el
empaquetador copie el binario del emisor al instalador. Eso sólo lo cierra una
comprobación **sobre el artefacto**, y ésa es PA-12 y no existe (RPT-025 §61).

La especificación exige, por tanto, una comprobación de empaquetado que falle si
el artefacto contiene un ejecutable llamado `eje-manifiesto`, una semilla de
firma o cualquier fichero de `reposo_semilla`.

### 3.1. Un binario no puede verificarse a sí mismo

Se propuso que el agente compruebe al arrancar que es el binario firmado
original. **Eso no prueba nada, y conviene decir por qué antes de construirlo.**

Quien puede sustituir el binario puede sustituir también la comprobación. El
binario reemplazado ejecuta *su* verificación, dice «conforme» y sigue. Es el
argumento de RPT-038 §2 con otro sujeto: una firma no sirve cuando la clave
—o el verificador— vive donde el atacante escribe.

En el vocabulario de RPT-006 §4, una autocomprobación sólo puede producir
honestamente **`ComprobacionImposible`**. Presentarla como `Conforme` es
convertir una garantía de papel en una pantalla verde, que es peor que no
tenerla: alguien dejará de mirar por otro lado.

Lo que sí verifica de verdad, en orden de lo que existe a lo que no:

| Dónde | Qué prueba | Estado |
|---|---|---|
| Gestor de paquetes al instalar | Que el artefacto viene del repositorio firmado | Depende de PA-14a y PA-12. **No existe** |
| Un verificador **fuera del proceso** (unidad de servicio, agente de gestión, arranque medido) | Que el fichero en disco es el firmado | **No existe.** Queda como PA-111 |
| El propio binario | Que un binario dice de sí mismo lo que su autor quiso | Nada |

La cadena de custodia del artefacto no la cierra el agente. La cierra el
repositorio firmado y lo que lo instala, y ninguno de los dos está construido.

## 4. `SinColector` sólo se puede declarar hacia adentro

Aquí hay una circularidad que conviene decir antes de construir nada.

**Un agente sin colector no puede avisar de que no tiene colector.** El aviso
viajaría por el canal que no existe. Es el mismo límite que hace que
`salidaNoDisponible` no viaje por syslog (RPT-032 §4) y que impide a una consola
que sólo lee del colector saber que el colector se cayó (RPT-051 §2C).

Así que la declaración va a los cuatro sitios donde **sí** se puede ver, y ninguno
es la sala:

1. **El instalador**, con la frase completa y no un código: «este sensor no
   late; nadie fuera notará si se apaga».
2. **La salida del proceso** en cada vuelta, que en despliegue es `journald`.
   Ya implementado en RPT-053 §2.
3. **El IPC**, para que VIS-04 lo pinte al técnico en sitio. **No implementado.**
   Ver §5.
4. **El registro de evidencia**, con un asiento al arrancar. Es el único de los
   cuatro que sobrevive al reinicio y viaja con la evidencia a una auditoría.
   Propuesto, no decidido: exige una clase de evento nueva.

## 5. Consecuencia concreta: la décima condición

Hoy `SinColector` vive sólo en `Resultado.latido`, que va a pantalla y a ningún
sitio más. VIS-04 no lo ve, porque VIS-04 lee `Condiciones` por IPC.

Se propone `sinColector` como **décima condición**. Encaja en la definición de
RPT-019 §2 —las condiciones son verdaderas hasta que alguien interviene— mejor
que casi ninguna: es un estado de configuración, no un suceso.

Y es la **segunda condición no emisible**, por la razón del §4. `EMISIBLES` pasa
a ser ocho de diez, y la prueba de PA-91 —que obliga a que toda condición salga
al SIEM salvo las que no pueden— tiene que admitir la segunda excepción de forma
explícita, nunca por omisión.

Esto toca los seis sitios del manifiesto como fuente única: `contrato-ipc.toml`,
`eje-ipc/mensajes.rs`, `puente.ts`, `CAMPOS_CONDICIONES`, la salida de syslog
—donde consta como **excluida**— y VIS-04. Las pruebas de paridad ya existentes
son las que impiden que se quede a medias, que es cómo se han quedado a medias
diez mecanismos en este proyecto.

## 6. Privilegios, y qué corre como quién

| Componente | Usuario | Por qué |
|---|---|---|
| `eje-agente` | root o `CAP_NET_RAW` | Captura tramas. Sin eso no hay producto |
| Socket IPC | `0660`, grupo del operador | PA-82. `0600` obligaría a que la consola corriera como root |
| VIS-04 | usuario sin privilegios | Una consola gráfica con permisos de captura es superficie regalada |

El instalador tiene que crear el grupo y meter al operador en él, porque
`--grupo-ipc` toma hoy un **número** de grupo y no un nombre. Eso es PA-84 en su
significado verdadero, y el empaquetado es lo que lo vuelve visible: un
instalador que escriba un GID a mano se rompe en cuanto la distribución asigna
otro.

## 7. Arranque, supervisión y el latido

La unidad de servicio es PA-65 y sigue sin escribirse. Con el latido cableado
adquiere una exigencia que antes no tenía: **el supervisor debe reiniciar el
agente, no sólo lanzarlo.** Un agente que muere y no vuelve deja de latir, y a
partir de PA-105 eso será una llamada — correcta, pero evitable.

El intervalo del latido es hoy `INTERVALO_LATIDO_MS = 60_000`, hipótesis
declarada y no medida (PA-41). Es el **primer parámetro que exige configuración
firmada** (PA-79): si viaja en el latido para que el colector no tenga que
suponerlo, no puede salir de un argumento de línea de órdenes que cualquiera
cambia en el equipo comprometido.

## 8. Lo que sólo se prueba sobre el artefacto

Ninguna de estas cinco se puede comprobar desde `cargo test`, y por eso PA-12
lleva abierto desde RPT-002:

1. El artefacto **no** contiene `eje-manifiesto` ni material de firma (§3).
2. El binario arranca con la configuración que instala el propio instalador, no
   con la del desarrollador.
3. Sin colector, el instalador **imprime la frase** del §4.1. Se comprueba
   leyendo su salida, no leyendo el código que la produce.
4. El servicio se reinicia tras matarlo (§7).
5. El grupo del socket existe y el operador pertenece a él (§6).

Son pruebas de proceso sobre la plataforma de destino. Hasta que existan, el
empaquetado está descrito y no verificado, y este reporte no dice lo contrario.

## 9. Lo que este reporte no resuelve

**No cierra PA-104.** Un sensor que se instala, late y declara su estado sigue
sin tener a nadie que note su ausencia. RPT-052 §6 lo dejó dicho y sigue vigente.

**No decide el formato del paquete.** `.deb`, `.rpm`, tarball o imagen depende
del repositorio firmado, que es infraestructura pendiente y no código.

**No cubre la consola de sala.** Es el tercer artefacto y su suministro de datos
es el colector; hoy no existe ni tiene reporte.

## 10. Puntos abiertos

| ID | Punto |
|---|---|
| **PA-107** | Este reporte. Se cierra cuando el artefacto existe y pasa las cinco pruebas del §8 |
| ~~PA-109~~ | ✅ La décima condición `sinColector`, implementada y verificada en RPT-055 |
| **PA-110** | ¿Un asiento de arranque que declare la ausencia de colector? Exige clase de evento nueva (§4.4) |
| **PA-111** | Verificación del artefacto **desde fuera del proceso**. El binario no puede hacerlo por sí mismo (§3.1) |
| PA-12 | Comprobaciones sobre el artefacto. Abierto desde RPT-002 |
| PA-65 | Unidad de servicio con reinicio, no sólo arranque (§7) |
| PA-79 | Configuración firmada. El intervalo del latido es su primer parámetro |
| PA-84 | `--grupo-ipc` por nombre y no por número (§6) |
| ~~PA-104~~ | ✅ Cerrado por observación en RPT-057 §4 |
| ~~PA-105~~ | ✅ `eje-vigia`, RPT-057 |
| PA-106 | Paridad entre lo que ve el técnico por IPC y lo que verá la sala por syslog |

---

*Reporte Nº 54 — Empaquetado dual · PremosCorp · 13 de agosto de 2026*
