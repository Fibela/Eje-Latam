# RPT-092 — La frase que se quemó

**Tema:** PA-144. Una credencial real expuesta por el orden de dos sentencias
**Nº de reporte:** 092
**Fecha:** 31 de agosto de 2026
**Área designada:** Herramientas
**Entidad:** PremosCorp
**Estado:** **Cerrado** en cuanto al defecto. **Abierto** en cuanto a sus consecuencias: PA-145, PA-146, y PA-53 escalado

- **Depende de:** RPT-026 §5 (donde nació PA-53), RPT-082 (PA-134, el defecto anterior de la misma función), RPT-090 §5.1 (para qué valen los lectores de texto)
- **Aborda:** PA-144 (cerrado). Abre PA-145 y PA-146. Escala PA-53

---

## 1. Esto no es un hallazgo de auditoría

Los noventa y un reportes anteriores documentan defectos encontrados mirando. Éste
documenta uno que **se cobró una credencial** antes de que nadie lo estuviera buscando.

Aprovisionando la VM de PA-78 se ejecutó:

```
cargo run -p eje-manifiesto -- configurar --semilla clave.sem …
Frase de paso (la de esta semilla), y Enter al terminar.
AVISO: se vera al teclearla; no la use delante de nadie (PA-53).
<la frase, en claro>
Error: Fichero { ruta: "clave.sem", kind: NotFound }
```

La frase se tecleó, se vio, quedó en el registro de la sesión. **Y no servía para nada:**
el fichero que la iba a usar no existía, y eso se sabía antes de preguntar.

Hubo que darla por comprometida.

## 2. El mismo binario llevaba los dos órdenes

| Orden | Qué hacía |
|---|---|
| `generar` | Comprobaba el fichero **y luego** pedía la frase |
| `emitir` | Pedía la frase, y luego leía |
| `configurar` | Pedía la frase, y luego leía |

`generar` tiene la guarda porque negarse a sobrescribir una semilla era una decisión de
diseño consciente —una semilla pisada deja huérfano todo lo firmado— y esa guarda tenía
que ir antes de cualquier otra cosa. **El orden correcto estaba en el fichero, aplicado a
un solo caso, por un motivo que no era éste.** Nadie lo generalizó porque nadie se hizo la
pregunta hasta que costó algo.

## 3. Avisar no es una contramedida

Lo más incómodo del incidente: el aviso de PA-53 describe el riesgo con exactitud, se
imprimió, se leyó, y la frase se tecleó igual. Porque no hay alternativa — la herramienta
no ofrece otra forma de dar la frase.

Un aviso que nombra un peligro sin ofrecer salida no reduce el riesgo: **traslada la culpa
del diseño a quien lo usa.** PA-53 pasa a 🔴 por esto, no por el incidente.

## 4. Lo entregado

Sale `entradas_sin_secreto(ruta_semilla, ruta_entrada)`, que lee las dos entradas, y
`emitir` y `configurar` además analizan el TOML antes de preguntar: un fichero mal escrito
tampoco quema una frase.

Es una **función** y no dos líneas movidas a propósito. Mover las líneas arregla los dos
sitios de hoy y ninguno de mañana; existiendo la función, quien añada el cuarto comando
que abra una semilla se encuentra el orden correcto ya escrito y con su motivo al lado.

## 5. La barrera vive en xtask, y eso no es comodidad

Lo que hay que comprobar es **el orden de dos sentencias**. El compilador no lo ve; es
justo el hueco para el que valen los lectores de texto (RPT-090 §5.1).

Y un lector de texto sobre fuente Rust necesita quitar comentarios primero —esta prueba
nombra las dos agujas en su propia prosa, y la documentación de `entradas_sin_secreto`
explica el defecto citándolas—. El único analizador de comentarios del proyecto, el que
sabe de bloques anidados, cadenas crudas y literales de carácter, está en
`xtask/src/exclusion.rs`. Escribir un segundo en `eje-manifiesto` sería exactamente la
duplicación contra la que avisa la documentación de `Recorrido`: **bastaría con que una
copia se quedara atrás para que la barrera empezara a mentir.**

### 5.1 La escribí mal a la primera

La primera versión comparaba primeras apariciones **globales**. La definición
`fn pedir_frase(` está en la línea 134, muy por encima de cualquier llamada, y la primera
llamada real vive en `generar`, que va antes que `emitir`. Habría salido roja con el
código ya correcto.

La versión buena acota por función —`fn ` en columna cero delimita el cuerpo—, salta el
bloque `#[cfg(test)]`, y **falla en voz alta si cualquiera de las dos agujas desaparece**
en lugar de quedarse verde sin nada que comprobar. Es la misma aserción anti-vacuidad de
RPT-090 §4, tercera vez que hace falta.

## 6. Tres cosas más, encontradas al escribir esto

**`.gitignore` no protegía la semilla.** No había línea para `*.sem`. `generar` se ejecuta
desde la raíz del árbol y escribe ahí: sin esa línea, el primer `git add -A` mete el
secreto raíz del despliegue en el índice. La barrera real es gitleaks, y un blob cifrado
con Argon2id no tiene por qué parecerle una credencial. Añadido, junto con los artefactos
emitidos —que no son secretos, pero son de **una** máquina, y versionarlos invita a
instalar el de otra.

**Cuatro ficheros de cero bytes versionados en `xtask/src/`,** con nombres como
`conformidad.rs350:8` donde ese `:` es U+F03A. Salieron de una redirección de shell contra
una salida de `grep` en Windows. No rompen nada; están commiteados en un repositorio que
se audita a sí mismo.

**PA-145.** `Some(ruta) if ruta.exists() => …, _ => None` para `--anterior`: una errata en
la ruta rebobina la serie a 1 **en silencio**, en las dos órdenes. Es RPT-006 §4 en un
sitio nuevo. Se separa a propósito: una cosa por reporte.

## 7. La consecuencia grande: PA-146

La semilla de la VM no apareció. Sin ella, esa configuración firmada **no se puede volver
a emitir**, así que la identidad del sensor se rota entera. Y ahí aparece lo que no
teníamos:

| Inventario | Centinela | Lectura de `arranque.rs` |
|---|---|---|
| ausente | sin establecer | primer arranque legítimo |
| ausente | establecido | **supresión** |
| presente, no verifica | cualquiera | **manipulación** |

Cambiar `clave-cliente.pub` deja cualquier `inventario.inv` anterior sin verificar →
manipulación. Borrarlo dejando el centinela → supresión. **Las dos lecturas son correctas.**
El almacén no distingue una rotación legítima de un ataque porque nadie se lo ha dicho
nunca.

Hoy la operación son cuatro `rm` a mano de un operador con prisa, indistinguibles de
aquello que el almacén existe para detectar. Es la maniobra que hay que hacer bien
justamente el día que una semilla se compromete — que es el día de hoy, en pequeño.

## 8. Lo que este incidente enseña y no estaba escrito

Los mecanismos sin cablear son el defecto dominante de este proyecto. Éste es una variante
que no habíamos nombrado: **un mecanismo cableado en un sitio y no en los otros dos, con la
razón correcta escrita para el sitio equivocado.**

No lo caza la paridad —no hay dos lados que comparar—, ni el compilador —los dos órdenes
compilan—, ni una revisión a ojo —las tres funciones se leen bien por separado—. Lo cazó
usar la herramienta.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| PA-144 | **Cerrado.** §4 y §5 |
| PA-53 | **Escalado a 🔴.** §3 |
| PA-145 | Abierto. §6 |
| PA-146 | **Abierto, crítico.** §7 |
| PA-78 | Mitad B, esperando a que la rotación se ejecute |

---

*Reporte Nº 92 — La frase que se quemó · PremosCorp · 31 de agosto de 2026*
