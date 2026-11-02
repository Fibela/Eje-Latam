# RPT-108 — El sabotaje que no se puede contar

| | |
|---|---|
| **Tema** | El sabotaje que no se puede contar |
| **Número** | 108 |
| **Fecha** | 2026-10-27 |
| **Área designada** | Seguridad |
| **Abre** | PA-169 (bloqueante) |
| **Cierra** | PA-162 |
| **Precedentes** | RPT-077 §5 (la doctrina de «no manda nadie»), RPT-052 §1 (el silencio), RPT-105, RPT-107 |

---

## 1. El hallazgo, en una frase

**`configuracionNoVerifica` está declarada emisible y es la única manipulación del
sistema que no puede viajar, porque destruye su propio camino de salida.**

```
salida.rs:579   ("configuracionNoVerifica", true)        ← emisible, urgente (107)
main.rs:713     Configuracion::NoVerifica(_) => Efectivas { interfaz: None, syslog: None, … }
main.rs:671     syslog: colector_declarado(&valores.colector)   ← el colector sale de la CONFIG
```

Una parte del diseño afirma que esa condición **tiene que llegar al colector**. La
otra hace imposible que llegue: la dirección del colector vive dentro del fichero
que acaba de romperse.

No es una decisión. Es una contradicción entre dos partes del diseño, y nadie la
había notado porque nadie la había medido.

---

## 2. Cómo se midió, y de quién es el hallazgo

Es de RUA, y vino por un camino que conviene contar porque el camino es la mitad
del valor.

El laboratorio tenía pendiente el escenario `configuracionNoVerifica` (107) de su
deuda D2, y se le dijo que no necesitaba esperar al `centinela.dat`: se monta
volteando un byte de `agente.conf.firmado` sobre el snapshot de la golden.

Se le dio además una predicción, y **la predicción estaba mal**. Se afirmó que el
latido seguiría saliendo con `intervaloMs=60000` —el valor del código— porque con
la configuración inválida «no manda nadie» y el agente caería al valor por
omisión. Se añadió que si aparecía 30000 habría un defecto grave, porque
significaría que una configuración que no verifica sigue dictando parámetros.

La máquina contestó otra cosa. El banner post-corrupción:

```
Configuracion      : NO VERIFICA
     la configuracion NO verifica: alguien la toco o la firmo otra clave
  !! Hay un fichero de configuracion y este agente NO lo acepta.
     No se cae a la linea de ordenes: a quien pudo tocar el fichero
     le bastaria romperlo para recuperar el mando por argumentos.
     Este agente no vigila nada y no emite nada. Solo lo declara.
Interfaz           : NINGUNA; este sensor no esta vigilando nada
Captura            : NO SE INTENTA
Salida de alertas  : NINGUNA; las alertas no salen del equipo
```

El agente **no cae al valor del código: se niega entero.** No hay latido, luego no
hay `intervaloMs` que medir. La premisa de la predicción —«cae a defaults»— era
falsa, y el diseño real es más seguro que el modelo que se le dio al laboratorio.

Dos cosas se confirmaron: el banner imprime el motivo de la configuración —la
asimetría que PA-163 arregló para el inventario, vista desde el lado que ya
funcionaba— y la condición se enciende.

### 2.1 La pregunta que abrió el punto

RUA no se detuvo en el verde. Leyó `Salida de alertas : NINGUNA` y preguntó:

> *¿Es intencional que un tamper de config llegue a ASC sólo como silencio, sin
> una alerta de tamper out-of-band?*

Y añadió que no lo concluía, que era terreno nuestro. **Esa pregunta es el
hallazgo.** La medición sola se habría archivado como «D2 cerrado, escenario
verde»; la pregunta sobre lo que la medición *no* mostraba es lo que llevó al
código, y en el código la contradicción está en tres líneas.

---

## 3. Por qué es grave

Todas las demás manipulaciones dejan intacto el camino por el que se cuentan:

| Manipulación | Condición | ¿Viaja? |
|---|---|---|
| Borrar el centinela | `operativaNoAprovisionada` + `recuperacionNoAprovisionada` | Sí — medido por RUA el 23-oct |
| Sustituir la clave operativa | `operativaRotada` / `operativaRevocada` | Sí |
| Revertir el inventario | `inventarioNoVerifica` | Sí — medido el 20-oct |
| Borrar el inventario | `inventarioSuprimido` | Sí |
| **Romper la configuración firmada** | **`configuracionNoVerifica`** | **No** |

Un adversario que quiera cegar un sensor **sin levantar ninguna alarma externa**
tiene exactamente un movimiento, y es romper la configuración firmada. El sensor
lo declara todo —con detalle, con motivo, honestamente— en su propio diario, que
es precisamente donde el adversario ya está. Fuera sólo se ve silencio, y RPT-052
§1 ya dejó escrito que la sala no sabe distinguir un sensor en silencio de uno
desenchufado.

El banner lo dice sin saber que lo dice. Su propia línea para el caso sin
colector es: *«NINGUNA; las alertas no salen del equipo y nadie fuera notara si se
apaga»*.

---

## 4. Lo que **no** es el arreglo

**La negativa a caer a la línea de órdenes es correcta y no se toca.** Su
justificación está en el propio banner y es sólida: *«a quien pudo tocar el
fichero le bastaria romperlo para recuperar el mando por argumentos»*. Volver a
admitir `--syslog` sin más reabre exactamente eso.

Esa asimetría con `configuracionSinFirmar` —donde la línea de órdenes **sí**
manda— es deliberada y hay que conservarla: que falte la configuración es un
estado legítimo de despliegue; que esté y no verifique, no.

---

## 5. Los dos caminos que no reabren nada

**(a) Anclar el colector donde el tamper no alcanza.** El centinela existe
precisamente para sobrevivir a que toquen el almacén, y ya guarda cosas que deben
sobrevivir. Si la dirección del colector viviera ahí, romper la configuración no
borraría el camino de salida. Cuesta una migración de formato —v4 → v5— y hay que
decidir qué pasa con un sensor cuyo centinela es anterior.

**(b) Admitir `--syslog` sólo como destino de la declaración de avería.** Sin
habilitar interfaz, ni captura, ni perfil, ni grupo, ni ningún otro parámetro: un
argumento que únicamente permite al agente **acusar** y nunca **obedecer**.

La segunda es más barata y su asimetría se sostiene: quien aporta ese argumento no
recupera el mando, sólo le da al sensor un sitio donde delatarle. Un atacante que
rompa la configuración y además pase `--syslog` está pagando por que su sabotaje
se cuente. La objeción que hay que resolver antes de elegirla es cuál es el
comportamiento cuando **nadie** lo pasa, que es el caso del despliegue real: el
silencio sigue ahí, sólo que ahora es evitable por quien configura el servicio.

Eso empuja hacia la (a) como arreglo de verdad y la (b) como paliativo. No se
decide en este reporte.

---

## 6. PA-162, cerrado

Entra `eje-manifiesto aceptar-inventario`. El razonamiento está en RPT-105 y
RPT-107 §2 y no se repite. Lo que cierra el punto son **siete pruebas en verde y
cinco perturbaciones vistas en rojo**, cada una atribuida a una sola causa.

De esas cinco, la segunda no estaba en el plan de RPT-105 §8. Salió de que la
primera —retirar la guarda de reversión— hizo caer la prueba por el `expect_err` y
no por la aserción del centinela, que es lo que se había predicho. Preguntarse por
qué reveló que **dos de las tres aserciones de esa prueba seguían sin medir**: la
perturbación validaba «no se negó» y dejaba intactas «se negó pero escribió igual»
y «la marca bajó».

Se añadió entonces la perturbación de **mover la guarda a después de escribir**, y
ahí se vio lo que importa, byte a byte:

```
left:  … 0, 4, 1, 0,0,0,0,0,0,0, 3, …   ← la marca bajó a 3
right: … 0, 4, 1, 0,0,0,0,0,0,0, 7, …   ← estaba en 7
```

Todo lo demás idéntico, las anclas incluidas. El ataque de reversión ejecutándose,
y la aserción que lo caza validada por fin.

**La lección del día sobre el método:** una perturbación que pone roja la prueba
correcta no valida la prueba — valida **la aserción que falló**. Las demás siguen
sin medir, y hay que perturbar una vez por aserción que se quiera sostener.

---

## 7. Lo que queda

- **PA-169**, bloqueante. Decidir entre (a) y (b) del §5.
- El `centinela.dat` del 7b para RUA, que sale de la ceremonia real y pasa por la
  frase de paso del administrador.
- **PA-165**, abierto hasta ver una corrida de CI en verde.
- **PA-167** (la tabla `ORDENES` derivada), **PA-164**, **PA-160**, **PA-152** y
  la tarea #30, donde estaban.
- La predicción del `sello=` para el 7b está escrita y es `sello=0000…0000`,
  `asiento=0`, sin `sello-de-evidencia`: un sello exige `ultimo > 0` y los
  asientos los producen las alertas, no el aceptar un inventario.
