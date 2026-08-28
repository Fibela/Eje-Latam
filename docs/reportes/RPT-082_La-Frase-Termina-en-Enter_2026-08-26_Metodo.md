# RPT-082 — La frase termina en Enter

**Tema:** PA-134. `pedir_frase` leía hasta el fin de la entrada, no hasta el salto de línea
**Nº de reporte:** 082
**Fecha:** 26 de agosto de 2026
**Área designada:** Método
**Entidad:** PremosCorp
**Estado:** Construido y probado. Cierra PA-134

- **Depende de:** RPT-079 (donde apareció, aprovisionando la VM), PA-53 (la frase se ve al teclearla)
- **Aborda:** PA-134

---

## 1. Tres intentos, y en dos pareció culpa del operador

`pedir_frase` usaba `read_to_string`, que lee **hasta el fin de la entrada** y no
hasta el salto de línea. Pulsar Enter no terminaba nada: `eje-manifiesto` se quedaba
esperando, sin decir por qué, y hacía falta un Ctrl-D que el aviso no menciona.

Costó tres intentos aprovisionando la VM de PA-78. Durante los dos primeros la
lectura razonable fue «lo estará escribiendo mal», que es lo que un programa colgado
sin mensaje induce a pensar de quien lo usa.

## 2. El segundo filo, que es el peligroso

Cualquier línea pegada detrás **entraba en la frase**.

Las dos órdenes del aprovisionamiento se pegan juntas de forma natural —`generar` y
luego `configurar`—, y con `read_to_string` la segunda se convertía en parte de la
frase de paso de la primera. La semilla habría quedado cifrada con el texto de un
comando.

Y no se habría sabido ahí. Se habría sabido después, cuando `configurar` fallara con
*«una frase distinta no abre la semilla»*: un mensaje correcto apuntando a la causa
equivocada.

La prueba `una_orden_pegada_detras_no_se_convierte_en_la_frase` reproduce el pegado
exacto de aquel día.

## 3. Una línea, y sólo `\r` y `\n`

Una frase con un salto de línea dentro no se puede teclear, así que admitirla no
compraba nada y era justo la puerta por la que entraba el texto pegado.

Se cortan `\r` y `\n` del final, y **sólo** esos. Recortar espacios alteraría en
silencio un secreto que alguien eligió con ellos, y el fallo aparecería mucho
después, al no poder abrir la semilla.

Y sigue funcionando por tubería —`printf '%s' 'frase' | …`—, que es como se desbloqueó
el aprovisionamiento y la forma reproducible: no depende de que nadie recuerde
Ctrl-D.

## 4. La causa raíz no era la función: era que nadie podía mirarla

`pedir_frase` leía `std::io::stdin()` directamente. Ninguna prueba podía ejercitarla
sin un terminal, así que el defecto vivió desde que se escribió el emisor.

Es la misma forma que PA-132, donde la ruta del socket vivía dentro de un fichero que
importa Electron: **una constante o una lectura que ninguna prueba puede mirar es una
que se queda atrás**.

La lectura sale a `leer_frase(&mut impl BufRead)`. El crate `eje-manifiesto`, cuyo
binario corría **cero** pruebas, pasa a cinco.

## 5. Y una decisión de los tres estados

Entrada cerrada sin entregar nada **no es una frase vacía**: es que nadie llegó a
escribir. Colapsarlas dejaría que un guion mal encadenado sellara una semilla sin que
nadie hubiera decidido con qué.

Cero bytes da error. Una línea vacía se lee, y que esté vacía lo rechaza el sellado,
donde ya estaba decidido (`una_frase_vacia_no_sella_nada`).

## 6. Y la mitad que se ve

El aviso decía cómo se **ve** la frase y no cómo se **termina**:

```
Frase de paso (nueva, para cifrar la semilla), y Enter al terminar.
AVISO: se vera al teclearla; no la use delante de nadie (PA-53).
```

Decir qué hacer no cuesta nada. Que alguien lo averigüe a la tercera, sí.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-134~~ | ✅ **Cerrado** |
| PA-53 | Sin cambios. La frase se sigue viendo al teclearla |

---

*Reporte Nº 82 — La frase termina en Enter · PremosCorp · 26 de agosto de 2026*
