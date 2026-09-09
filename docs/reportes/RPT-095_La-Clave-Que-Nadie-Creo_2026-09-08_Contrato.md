# RPT-095 — La clave que nadie creó

**Tema:** PA-146a. La condición catorce, y el aviso que llevaba treinta y tres días sin destinatario
**Nº de reporte:** 095
**Fecha:** 8 de septiembre de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Cerrado por observación en máquina real.** Captura en `docs/evidencia/RPT-095_Sin-Clave-De-Recuperacion_2026-09-08.png`

- **Depende de:** RPT-015 §4 (las dos claves), RPT-092 (la semilla perdida), RPT-094 (el aviso que no era contramedida), RPT-091 §4 (la cadena que hizo el trabajo)
- **Aborda:** PA-146a (cerrado). Parte PA-146. Desbloquea PA-146b. Corrige RPT-094 §7

---

## 1. El punto cambió al mirar el almacén

PA-146 se abrió como «la rotación de identidad no tiene procedimiento». Antes de diseñar el
certificado se miró qué hay en el sensor desplegado:

```
/var/lib/eje-latam/
  centinela.dat   clave-cliente.pub   evidencia.alm   evidencia.anc
```

**No hay `clave-recuperacion.pub`.** Y el certificado de rotación se firmaría con esa clave.

La maquinaria existe entera: hay un hueco reservado en `RutasAlmacen`,
`eje-manifiesto recuperacion` la genera y la reparte entre tres custodios, y
`arrancar_con_almacen` la carga con su propio dominio criptográfico —con un comentario que
explica por qué no puede sustituirse por la operativa, porque hacerlo abriría el ataque de
PA-27—. Está todo pensado, y nadie la creó.

`generar` lo avisa cada vez que se ejecuta. Yo mismo leí ese aviso el 2 de septiembre al
dictar el bloque de aprovisionamiento, y escribí que era «correcto y hoy no lo atendemos».

**Es la forma de PA-53 otra vez**, y por eso PA-146 se parte: no tiene sentido diseñar un
certificado que se firma con una clave que la mitad de los despliegues no tiene y que nadie
sabe que le falta.

## 2. El hecho existía y moría en una función

`arrancar_con_almacen` cargaba la clave, la usaba para las revocaciones, y devolvía
`(EstadoArranque, Centinelas)`. **El tercer hecho que ya tenía en la mano se tiraba.**

Ahora devuelve `Arranque`, un tipo con nombre. Como tercer elemento de una tupla habría sido
un `bool` desnudo que en el destino no dice de qué habla.

## 3. Tres decisiones que no eran mecánicas

**Se llama `sinClaveDeRecuperacion` y no `riesgoCompromiso`.** El equipo propuso el segundo.
Ninguno de los trece campos del contrato nombra un juicio: todos nombran un hecho
observable. El agente sabe que un fichero no está; que eso signifique «nodo desechable ante
compromiso» es la conclusión, y la conclusión la compone VIS-04 (RPT-088 §2). Y no
`sinRecuperacion` a secas: en este proyecto «recuperación» podría leerse como recuperar el
almacén, los respaldos o la serie. Lo que falta es una **clave**, y el nombre apunta al
fichero.

**Entra en `Ciclo` por el constructor, no por un `declarar_*`.** Los otros tres hechos de
fuera se declaran porque cambian o se averiguan después: la escucha se abre más tarde, la
captura puede caerse en marcha, la configuración se relee. Éste se sabe al leer el almacén y
no cambia mientras el proceso vive. Con un `declarar_*` de valor inicial `false`, un `main`
que lo olvidara diría «sí hay clave de recuperación» — el supuesto optimista contra el que
avisa el propio comentario de `configuracion`. Por el constructor, **olvidarlo no compila**.

**Sale por syslog**, sin acusar a nadie. `EMISIBLES` pasa de 11 a 12. El argumento es que es
la única condición que el operador del sensor **no puede resolver**: la ceremonia vive en
otra máquina y con tres custodios, así que decírsela sólo a él es decírsela a quien no puede
hacer nada. La sala sí sabe cuántos sensores de su flota están sin salida. Y no inunda: es
una transición, se emite una vez por arranque.

### 3.1 Y un booleano basta, comprobado y no supuesto

Aquí normalmente saltaría la doctrina de los tres estados. No aplica, y por una razón que se
fue a leer antes de escribir el campo: `arrancar_con_almacen` propaga `ErrorArranque::Clave`
si el fichero existe y está mal formado, así que **el agente no llega a arrancar**. A un
agente vivo sólo le llegan dos estados.

Queda escrito en el propio campo qué hacer el día que eso cambie: si ese error se degradara
a `None`, este campo se parte en dos como lo están `inventarioSuprimido` e
`inventarioNoVerifica`.

## 4. Catorce sitios, y la cadena los pidió todos

| Lo pidió | Sitios |
|---|---|
| El compilador de Rust | `struct`, desestructuración de `enumerar`, firma de `condiciones`, `Ciclo::nuevo` ×4, nueve literales de prueba |
| Una prueba de Rust | `EMISIBLES`, el latido, `CAMPOS_CONDICIONES` |
| `tsc` con `satisfies` | `Condiciones` en `puente.ts`, `CALMA` en las pruebas |
| Una barrera de texto | la tabla de `vis04.ts`, `NOMBRES` en `diagnostico.js` |
| **Nadie** | **once comentarios que decían «las trece»** |

Los catorce primeros salieron solos. Los once últimos hubo que buscarlos con `grep`, y son
exactamente la clase de prosa que este proyecto lleva noventa reportes cazando: correcta
ayer, falsa hoy, y sin nada que lo note.

### 4.1 Dos fallos míos, de la misma familia

**El script escribió `false` en los nueve literales sin mirar qué significaba cada uno**, y
uno de ellos era `todas_encendidas()`, cuyo contrato entero es que **todas** estén a cierto.
Lo anoté al lanzarlo y no volví. Lo cazó `el_latido_nombra_lo_emisible_y_calla_lo_que_no_puede_salir`,
con un mensaje que dice lo que pasa —«es emisible y no viaja en el latido: la sala no lo
verá nunca»— en lugar de un `assert_eq` pelado.

**Y escribí un `rm -rf X && mv Y X` que destruye el original antes de comprobar que el
reemplazo llegó.** El `scp` murió por abrir cuatro sesiones seguidas, y durante un rato la VM
no tuvo ni la copia vieja ni la nueva. Es el mismo patrón de toda la semana —una operación
que da por hecho su propio éxito— esta vez en una línea de shell. La forma correcta lleva un
`test -f` delante, y así se dictó la segunda vez.

## 5. La barrera que dije que faltaba ya estaba

Tres veces esta semana partí una fila del tablero en dos —la nueva y una «(histórico)»— y
las tres lo vi al releer. En RPT-094 §7 escribí que eso metía un identificador duplicado y
que sólo no rompía la cuenta gracias a `un_identificador_repetido_solo_cuenta_una_vez`.

**Las dos afirmaciones son falsas.** `identificador_de` exige que el sufijo tras los dígitos
sea una sola letra minúscula; `PA-53 (histórico)` deja ` (histórico)` y devuelve `None`. Esas
filas nunca entraron en la cuenta, y la barrera que hizo el trabajo lleva escrito desde antes
de que yo llegara: *«un punto fantasma no se cierra nunca, porque no existe»*.

Diagnostiqué una barrera ausente sin leer la que estaba. Se anota aquí, y RPT-094 lleva
errata, porque **un reporte que acusa de un fallo inexistente hace que alguien construya la
defensa que ya tenía**.

## 6. La corrida, en la VM

Predicción escrita antes: catorce filas, dos en «sí». Salió exacto.

```
Sin clave de recuperación: si esta identidad se compromete, no hay forma de revocarla   sí
Sin colector: este sensor no informa a ninguna sala                                     sí
(las otras doce)                                                                       no
```

Y en el arranque del agente, antes de la pantalla:

```
!! SIN CLAVE DE RECUPERACION. Si la identidad de este sensor se compromete,
   no hay forma de revocarla ...
!! condicion ENCENDIDA: sinClaveDeRecuperacion
```

Un aviso que llevaba desde el 6 de agosto imprimiéndose en la máquina del administrador —y
que quien instala el sensor no ve nunca— llega por fin a la pantalla de quien opera y al
SIEM de quien vigila la flota.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| PA-146a | **Cerrado.** §6 |
| PA-146b | **Abierto, crítico.** El certificado de rotación, ya desbloqueado |
| PA-145 | `--anterior` inexistente rebobina la serie en silencio |
| PA-147 | La plantilla del instalador declara 30000 ms y el código 60000 |
| PA-148 | La cabecera de VIS-04 no dice qué sensor es |
| PA-142 | `diagnostico.js` sigue ciego, aunque hoy ganó la clave catorce |

---

*Reporte Nº 95 — La clave que nadie creó · PremosCorp · 8 de septiembre de 2026*
