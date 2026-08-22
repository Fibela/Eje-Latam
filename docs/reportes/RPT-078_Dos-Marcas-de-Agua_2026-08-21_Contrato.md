# RPT-078 — Dos marcas de agua

**Tema:** PA-79 paso 5. La secuencia de la configuración deja de viajar sin que nadie la mire
**Nº de reporte:** 078
**Fecha:** 21 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** Construido y probado. Cierra PA-79

- **Depende de:** RPT-017 (el centinela), RPT-012 §4.4 y PA-27 (la reversión del inventario), RPT-015 §6.1 y PA-33 (el techo de secuencia), RPT-074 §5, RPT-077 (la obediencia)
- **Aborda:** PA-79

---

## 1. El mecanismo estaba escrito y desconectado

`Valores::secuencia` viajaba firmada desde RPT-074. El emisor la incrementaba
leyendo la anterior verificada. Y **el agente no la comparaba con nada**.

Una configuración de la semana pasada —la del intervalo de latido largo, la que
apuntaba a otro colector— entraba sin resistencia. La firma es legítima; lo que no
lo es, es reponerla sobre un sensor que ya vio una posterior.

Es la familia dominante de este proyecto, esta vez en la defensa contra reversión.

## 2. Dos contadores, un fichero

El centinela existía para el inventario. La tentación era reutilizarlo tal cual, y
habría sido un error: un contador compartido acopla dos series que se emiten a
ritmos distintos. Publicar un inventario subiría la marca y dejaría fuera
configuraciones legítimas más bajas, y al revés.

Dos ficheros tampoco: RPT-017 ya dejó escrito lo que cuesta dispersar estado de
seguridad — una escritura sobrevive, la otra no.

Así que **dos marcas dentro de la misma escritura atómica**. O avanzan las dos o no
avanza ninguna.

## 3. El fallo que sólo aparece al meter la segunda marca

`aceptar_inventario` componía el fichero **entero** desde la secuencia del
inventario:

```rust
escribir_atomico(&rutas.centinela(), &serializar_centinela(secuencia))?;
```

Con una segunda marca dentro y un segundo escritor escrito de la misma forma,
aceptar un inventario **habría borrado la marca de configuración en silencio** — y
a partir de ahí cualquier configuración antigua y bien firmada entraría. El paso 5
habría cerrado la puerta y abierto una ventana al lado.

No es un descuido evitable leyendo con cuidado: es invisible mirando cualquiera de
los dos ficheros por separado. La corrección es estructural — los dos `aceptar_*`
**releen el disco y funden**, en lugar de componer el fichero desde lo que traen en
la mano. Un parámetro se puede pasar caducado; esto no.

Lo sujeta `las_dos_marcas_sobreviven_a_que_la_otra_avance`, en disco y en los dos
órdenes, porque uno solo dejaría pasar la mitad del error.

## 4. La compatibilidad hacia atrás habría sido la vulnerabilidad

El formato pasa a versión 2. La 1 llevaba una sola marca, y aceptarla sería aceptar
un fichero que **no dice nada** de la secuencia de configuración — es decir, que se
lee como «sin establecer».

Dieciocho bytes escritos a mano, y cualquier configuración antigua vuelve a entrar.

Se rechaza. Se puede hacer porque no hay ningún sensor desplegado; cuando lo haya,
migrar tendrá que ser una operación deliberada del emisor y no una lectura
tolerante del agente.

Por lo mismo, dentro del fichero:

- cada marca lleva **presencia y valor por separado**, o «sin establecer» y
  «establecida en cero» darían los mismos bytes;
- presencia en cero exige valor en cero, o habría dos formas de escribir lo mismo;
- un fichero que **no afirma ninguna** de las dos series se rechaza: eso ya lo dice
  la ausencia del fichero, y admitir las dos formas dejaría poner un centinela que
  no dice nada donde había uno que decía algo.

## 5. PA-33 aplicado aquí, antes de que lo pidieran

El inventario tiene techo de secuencia desde RPT-015 §6.1. La configuración no lo
tenía, y **la misma bala servía**: con la clave operativa se emite *una*
configuración con `u64::MAX`, el agente la acepta —la firma es válida— y ninguna
legítima puede ya superarla. El sensor queda congelado con lo que diga esa, para
siempre, y revocar la clave no lo arregla porque la marca sigue arriba.

Se rechaza como malformación, reutilizando `TECHO_SECUENCIA`. Lo que **no** tiene
todavía la serie de configuración es el camino de vuelta: `reiniciar_por` baja la
marca del inventario con un certificado de recuperación, y su corte está en espacio
de secuencia de inventario. Aplicarlo a la configuración sería volver a mezclar las
series. Queda como **PA-131**.

## 6. Por qué la frescura vive dentro de `analizar`

`analizar` recibe ahora la marca por parámetro. Podría haberse comprobado en el
agente, después de leer, con dos líneas.

No: un mecanismo que hay que acordarse de invocar acaba no invocándose — es la
lección de todo el proyecto, y este mismo campo es la prueba. Con la marca en la
firma de la función, **no se puede leer una configuración sin decir contra qué se
fecha**. Las quince llamadas de prueba que ahora escriben `sin_marca()` son el
precio, y es el precio correcto.

## 7. Anotar antes de obedecer, y no obedecer si no se pudo anotar

`aceptar_configuracion` avanza la marca **antes** de aplicar los parámetros, igual
que el centinela se escribe antes que el inventario. Si el proceso muere en medio,
queda la marca en N con la N sin aplicar; al rearrancar se vuelve a presentar con
`secuencia == aceptada`, que se admite porque la comparación es `<` y no `<=`.

Al revés —obedecer y después anotar— habría una ventana en la que el sensor corre
con una configuración cuya secuencia no consta. Ahí vive el ataque.

Y si la marca **no se puede escribir**, la configuración se degrada a
`NoVerifica`. No se obedece de todos modos: el próximo arranque no sabría que se
llegó a ver esta secuencia. El sensor ya sabe declarar ese estado y la sala ya sabe
leerlo (RPT-077 §5), así que no hace falta inventar nada.

## 8. Lo que esto no compra

Lo de siempre, y conviene repetirlo: la marca vive en el almacén que root
controla. Quien pueda escribir el centinela puede rebobinarlo, y con él la
configuración, de forma consistente. La protección completa exige un ancla fuera
del almacén escribible —contador monótono en TPM o elemento seguro—, y eso no está
disponible en todos los destinos.

Lo que sí se consigue es lo que RPT-017 ya decía del inventario: que revertir **no
sea silencioso**. Borrar el centinela es tan detectable como rebobinarlo, y ahora
además delata las dos series a la vez.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-79~~ | ✅ **Cerrado.** Los cinco pasos de RPT-074 §10 |
| PA-131 | **La serie de configuración no tiene camino de vuelta.** El techo impide el congelado permanente; falta el equivalente de `reiniciar_por` con un corte en espacio de secuencia de configuración |

---

*Reporte Nº 78 — Dos marcas de agua · PremosCorp · 21 de agosto de 2026*
