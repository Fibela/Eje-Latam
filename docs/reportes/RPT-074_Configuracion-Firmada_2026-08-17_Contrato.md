# RPT-074 — Configuración firmada

**Tema:** PA-79. Diseño: quién firma, qué cubre la firma y qué pasa en cada estado
**Nº de reporte:** 074
**Fecha:** 17 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** Diseño emitido antes del código. **Pasos 1 y 2 construidos** (§10)

- **Depende de:** RPT-015 §4 (las dos claves del cliente), RPT-017 (el centinela), RPT-047 (degradar en lugar de morir), RPT-070 (el precedente de la undécima condición), RPT-006 §4
- **Aborda:** PA-79

---

## 1. Qué compra esto, dicho sin exagerar

Hoy `agente.conf` está en `0640 root:root`. **Sólo root puede editarlo**, y root
ya puede sustituir el binario, parar el servicio o borrar la evidencia. Firmar la
configuración **no le cierra la puerta a root**, y decir lo contrario sería
vender humo.

Lo que cambia es **qué puede hacer en silencio**.

Hoy, una línea editada —`EJE_INTERVALO=3600000`— alarga la ventana de silencio
que la sala vigila, y todo sigue pareciendo sano: el latido llega, las once
condiciones en verde, el panel tranquilo. Con configuración firmada el agente no
la acepta, y la caída es **visible**.

Convierte un ajuste silencioso en una avería ruidosa. Es el mismo movimiento que
PA-104 y PA-125.

Y el vector más probable no es un atacante: es un técnico que sube el intervalo
porque el sensor «hace ruido».

## 2. La garantía descansa en el censo de la sala, y hay que decirlo

Si la configuración no verifica, el agente **no sabe cuál es su colector** —está
en la configuración—. Un sensor que no arranca **no puede contar que no arrancó**.

La caída sólo es ruidosa si `eje-vigia` tiene a ese sensor en su censo
(`--esperar MAQUINA/INTERFAZ`), que es lo que distingue `NuncaVisto` de «no
desplegado». Sin censo, un sensor que falló al arrancar es indistinguible de uno
que nunca existió.

**Esto es una condición de la garantía, no una nota al pie.** Se escribe aquí para
no descubrirlo en planta.

## 3. Firma la clave operativa del cliente

`DominioClave::Cliente`, la misma que firma marcados y declaraciones de VLAN.

**Por qué no PremosCorp.** El cliente no puede pedirnos un fichero cada vez que
cambia el nombre de una interfaz. Además nos metería en PA-14a, que está en rojo
por custodia de hardware.

**Por qué es viable hoy.** Todo existe ya: `clave_operativa` tiene su sitio en
`RutasAlmacen`, `arrancar_con_almacen` **ya la carga y verifica**, y
`eje-manifiesto` ya la aprovisiona. Es exactamente lo que le falta a PA-126 y
aquí sí está.

## 4. Formato: el mismo que el inventario, no un texto nuevo

Fichero binario con mágico, versión, campos con prefijo de longitud y firma
híbrida sobre los bytes canónicos — la disciplina de `inventario` y `clave`, que
ya tiene analizador probado contra entrada hostil y objetivo de `cargo-fuzz`.

**Por qué no TOML ni pares `CLAVE=valor`.** Meter un analizador de texto en el
camino que decide si el agente confía en su propia configuración es superficie
nueva en el peor sitio posible. El formato binario con prefijos de longitud es el
que este proyecto ya sabe defender.

**Quién lo emite:** `eje-manifiesto`, que es la herramienta del administrador del
cliente y ya custodia esa clave. Una orden nueva, no un binario nuevo.

**Coste asumido:** el administrador no puede leerlo con `cat`. A cambio, el
agente imprime la configuración vigente en su banner de arranque, que es donde
alguien la mira de verdad.

## 5. Qué cubre la firma

Todo lo que puede cambiar **qué hace el sensor o cuánto se le oye**:

| Campo | Por qué está firmado |
|---|---|
| `interfaz` | Apuntar el sensor a otro segmento |
| `perfil` | `ot` deshabilita la Capa B y el descubrimiento activo |
| `colector` | Vacío es legítimo (RPT-054 §1), pero tiene que ser **declarado** |
| `intervalo_latido_ms` | La ventana de silencio que la sala vigila |
| `grupo_ipc` | Quién puede consultar por el socket |
| `nombre` | La identidad del sensor en la sala |
| ~~`almacen`, `directorio_socket`~~ | ❌ **Retirados en RPT-077.** La clave que verifica esta configuración es `<almacen>/clave-cliente.pub`: firmar dónde está el almacén es firmar dónde se busca la clave que decide si creer la firma. El círculo apareció al cablear la obediencia, no al diseñar el formato |

**Y dos campos que no son configuración sino defensa:**

**`maquina_esperada`** — el `hostname` del equipo donde esta configuración es
válida. Sin él, root copia la configuración de un sensor tranquilo sobre uno
ruidoso y **las dos firmas son legítimas**. El agente compara con
`/proc/sys/kernel/hostname` y se niega si no coinciden.

**`secuencia`** — monotónica, anclada en el centinela que ya existe (RPT-017).
Sin ella, root reinstala una configuración **antigua y correctamente firmada**
—por ejemplo la que tenía un intervalo largo— y la firma la avala. Es el ataque
de reversión que el centinela ya resuelve para el inventario; aquí se reutiliza,
no se inventa.

## 6. Tres estados al cargar, no dos

```rust
enum Configuracion {
    Firmada(Valores),
    Ausente,
    NoVerifica { motivo: … },
}
```

`Ausente` es «todavía no se ha aprovisionado». `NoVerifica` es «hay una y miente».
Colapsarlas mandaría al operador a aprovisionar cuando lo que hay es alguien que
tocó el fichero — es la distinción de `inventarioSuprimido` frente a
`inventarioNoVerifica`, y se copia tal cual.

**Y por eso son dos condiciones, no una:**

- **`configuracionSinFirmar`** — el agente corre desde argumentos. Emisible.
- **`configuracionNoVerifica`** — hay fichero y no pasa la firma. Emisible, con
  la gravedad de la manipulación.

Duodécima y decimotercera. El precedente es PA-125: la situación existe en el
mundo, así que existe en el vocabulario.

## 7. Por qué **no** se muere si la configuración no verifica

`panic!` está prohibido por los lints del workspace, pero el motivo de fondo es
otro: con `Restart=always`, morir es entrar en bucle de reinicio hasta que
`systemd` marque `failed` — y el proceso **se lleva el socket local consigo**.

El técnico que va a la planta a averiguar qué pasa se encuentra un cadáver mudo.
Vivo y degradado, el agente le dice a la cara qué le ocurre. Es RPT-047 §4 otra
vez: morir ruidosamente se cambió por vivir declarando, y por las mismas razones.

## 8. Y por qué el modo sin firma no es un mock

Un mock **se hace pasar por** lo real y el defecto es la indistinguibilidad. Este
estado es lo contrario: enciende una condición que viaja por syslog, sale en el
latido y aparece en el panel. Nadie puede confundirlo con el estado verificado.

Si el agente arrancara con argumentos y **callara**, entonces sí sería de esa
familia.

**El riesgo real es otro y hay que nombrarlo:** que el estado degradado se vuelva
el normal. Si desplegar sin firmar fuera cómodo, todo el mundo desplegaría sin
firmar, la condición estaría siempre encendida y se aprendería a ignorarla —la
fatiga de alertas de PA-45— y la firma no se usaría jamás.

La defensa es estructural, no documental:

- `EnvironmentFile` **desaparece** de la unidad y la ruta del fichero firmado va
  fija en el binario.
- La unidad **no pasa ningún argumento de configuración**, con prueba que lo
  exige — igual que la de `--directorio-socket` en RPT-067 §5.

Con eso, un despliegue sin fichero firmado arranca, grita y **no captura nada**,
porque no sabe qué interfaz mirar. No es un atajo cómodo: es una instalación
visiblemente inútil.

**Lo que se conserva a propósito:** el agente lanzado a mano con argumentos, que
es la herramienta con la que se observaron PA-125 y PA-123 esta semana. La
alternativa estricta —negarse a arrancar sin firma— la mata, y ese coste es real.

## 9. Si los argumentos y el fichero firmado coinciden en la misma ejecución

Los argumentos **se rechazan**, y el agente sale con error de uso.

No se ignoran en silencio: un argumento ignorado es alguien creyendo que aplicó
un cambio que no aplicó, y ése es el mismo defecto que `--syslog ""` (RPT-064).

## 10. Orden de construcción

1. ✅ **El formato, su analizador y las pruebas contra entrada hostil.**
   `guardian_cc::configuracion`, quince pruebas incluido el arnés de mutación
   determinista. `guardian-cc` pasa de 146 a 161.
2. ✅ **La orden de `eje-manifiesto` que lo emite.** `configurar`, seis pruebas
   más. El administrador escribe TOML y sale binario firmado.
3. ✅ **Las dos condiciones nuevas, con sus seis sitios.** `configuracionSinFirmar`
   y `configuracionNoVerifica`, de once a **trece**. `EMISIBLES` de 9 a 11: las
   dos viajan, porque describen el sensor y no el canal de syslog.
4. ✅ **Hecho en dos mitades, y a propósito.**

   **4a** — el agente **lee, verifica y declara**, con lo que las condiciones son
   verdaderas desde el primer commit en lugar de esperar apagadas a que alguien
   las encienda, que es PA-69.

   **4b** — el agente **obedece** (RPT-077). Con configuración firmada mandan sus
   valores y cualquier bandera dictada aborta el arranque; sin ella manda la línea
   de órdenes, como toda la flota de hoy; con una que no verifica **no manda
   nadie**. `EnvironmentFile` fuera de la unidad, que ya no configura el sensor.

   Al cablearlo aparecieron dos cosas que el diseño no vio: el círculo del almacén
   —que sacó dos campos del formato— y que la versión estricta de «no verifica»
   habría dejado huérfana una condición recién estrenada.
5. ✅ **`secuencia` y `maquina_esperada` — las dos defensas de §5** (RPT-078).
   Iban al final a propósito: sin ellas el mecanismo ya valía para el técnico
   despistado, que es el vector probable; con ellas vale además contra alguien
   deliberado.

   El centinela lleva ahora **dos** marcas de agua en la misma escritura atómica,
   una por serie, y `analizar` exige que le pasen la de configuración — no se puede
   leer una configuración sin decir contra qué se fecha. Y también aquí apareció al
   cablear lo que el diseño no vio: `aceptar_inventario` componía el fichero entero
   y **habría borrado la marca de configuración en silencio**, cerrando esta puerta
   y abriendo una ventana al lado.

### Lo que los pasos 1 y 2 ya sujetan

**La secuencia se adelantó al paso 5 sin querer, y era gratis.** Estaba en el
formato desde el paso 1, y el paso 2 la calcula leyendo la anterior
**verificada** — el mismo mecanismo que `secuencia_siguiente` usa para el
inventario.

Conviene no confundir eso con estar hecho: **viajó firmada durante cuatro días sin
que nadie la comparase con nada**, y en ese tiempo una configuración antigua y
correctamente firmada entraba sin resistencia. Un campo en el formato no es una
defensa hasta que algo lo lee (RPT-078 §1).

**`maquina_esperada` también.** El formato la lleva y `analizar` la compara
**después** de la firma. El agente le pasa su `hostname` de verdad desde el paso 4.

**El recorrido completo tiene prueba desde hoy:**
`una_configuracion_recien_emitida_la_verifica_el_agente` firma con el emisor y lee
con **el mismo código que corre en el sensor**. Es la gemela de
`un_manifiesto_recien_emitido_lo_verifica_el_agente`, y existe por lo mismo: dos
implementaciones del formato divergen, y la divergencia se lee como manipulación.

### Lo que el paso 3 decidió al construirse

**Un solo dato, no dos interruptores.** `condiciones()` recibe
`EstadoConfiguracion` —tres estados— y deriva de él los dos booleanos. Con dos
interruptores independientes existiría el estado imposible en que ambos son
ciertos y habría que confiar en que nadie lo encienda. Es RPT-053 §2 otra vez:
quitar del tipo lo que el dominio no tiene.

**El ciclo arranca en `Ausente`, no en `Firmada`.** El estado de partida tiene que
ser el que **no** afirma nada bueno: si arrancara en `Firmada`, un `main` que
olvidara declararla dejaría a toda una flota diciéndose aprovisionada sin que
nadie lo hubiera comprobado.

**`configuracionNoVerifica` no es manipulación**, aunque la firma rota apunte a
ello. Una máquina ajena, una clave rotada o un disco corrupto dan la misma
condición y no son un ataque; mandar a respuesta a incidentes por un error de
despliegue es la fatiga de alertas de PA-45. Viaja con gravedad alta y sin acusar,
como `registroSaturado`, y el motivo va al diario.

**Y el agente compara contra el `hostname` del núcleo**, no contra
`opciones.nombre` —que `--nombre` cambia—. Usar aquel habría permitido hacer
verificar la configuración de otro sensor con un argumento, que es exactamente lo
que `maquina_esperada` impide.

**Una decisión que sólo apareció al construir:** el perfil mal escrito se
rechaza en lugar de caer a `corporativo`. `ot` deshabilita la Capa B y el
descubrimiento activo; una errata que degradase encendería en una planta lo que
RPT-002 apaga a propósito, y lo haría **con una firma válida encima**.

## 11. Lo que este diseño **no** resuelve

Que root pare el servicio. Nada en la máquina lo impide, y por eso la garantía
descansa en §2.

Tampoco la primera configuración: llega sin firmar por definición, igual que la
primera clave. Es el mismo problema de arranque de confianza que PA-126, un piso
más abajo, y se apoya en el aprovisionamiento de PA-49.

## 12. Puntos abiertos

| ID | Punto |
|---|---|
| PA-79 | Este diseño. Sin implementar |
| PA-49 | El aprovisionamiento de la clave operativa, del que depende §11 |
| PA-105 | El censo de la sala, del que depende la garantía de §2 |

---

*Reporte Nº 74 — Configuración firmada · PremosCorp · 17 de agosto de 2026*
