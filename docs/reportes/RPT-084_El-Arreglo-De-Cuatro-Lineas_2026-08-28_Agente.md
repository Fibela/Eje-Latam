# RPT-084 — El arreglo de cuatro líneas

**Tema:** PA-136 cerrado. La consola responde entre trama y trama, y la latencia pasa de no acotada a acotada
**Nº de reporte:** 084
**Fecha:** 28 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Cerrado por observación en máquina real.** Seis de seis canales responden

- **Depende de:** RPT-083 (la medición que lo diagnosticó), RPT-034 §4 (atender sobre lo persistido), RPT-006 §4 (los tres estados)
- **Aborda:** PA-136 y PA-141 (cerrados). Abre PA-140. Matiza PA-78. Aporta cifra a PA-97

---

## 1. Qué se cambió

Cuatro líneas dentro del bucle de captura, en `crates/eje-agente/src/main.rs`:

```rust
tramos.atender += {
    let marca = Instant::now();
    let atendidas = atender_pendientes(escucha.as_ref(), &ciclo, anteriores.as_ref(), &estado_agente);
    ...
};
```

`atender_pendientes` es un ayudante nuevo que construye los `Manejadores` y delega. Se
llama **dos veces por vuelta**: entre trama y trama con las condiciones de la vuelta
anterior, y al final con las recién calculadas. Antes sólo existía la segunda llamada.

El tramo `atender` pasa a **acumularse** en lugar de asignarse. Sin eso, el instrumento
habría vuelto a medir lo que no era —el mismo defecto de RPT-083 §2 con otro disfraz.

## 2. Lo que se sirve a mitad de vuelta ya estaba en disco

RPT-034 §4 exige que una consulta responda con lo persistido y nunca con lo que vive
sólo en memoria. Atender a mitad de vuelta lo cumple **sin tocarlo**: entrega el
registro ya escrito y las condiciones de la última vuelta completa.

Parecía una concesión —latencia contra frescura— y no lo era. Atender al final entrega
un dato fresco que llega once segundos tarde, así que la edad del dato en manos del
operador es la misma. La diferencia es que una versión responde y la otra se cuelga.

## 3. La primera vuelta: los tres estados en un sitio nuevo

Antes de que termine la primera vuelta no hay ninguna condición evaluada. La salida
cómoda era devolver las trece en `false`.

Eso diría **«este sensor está sano»** sobre un sensor del que todavía no se sabe nada.
Es la mentira exacta que RPT-006 §4 prohíbe y que ya costó la décima condición en
PA-118. `Manejadores.condiciones` pasa a ser `Option<&Condiciones>` y el canal rechaza
con motivo.

**Y el rechazo alcanza sólo al canal que lo necesita.** `consultar-alertas` y
`obtener-estado-agente` responden igual: las alertas están en disco desde antes de que
arrancara este proceso, y negarlas porque el ciclo no ha terminado sería confundir «aún
no lo he calculado» con «no lo tengo». Dos pruebas nuevas cubren las dos mitades.

## 4. La corrida, en la misma VM y con el mismo goteo

Agente en modo servicio sobre `lo`, configuración firmada, `ping -i 0.4` de fondo:

| Canal | Antes (RPT-083 §6) | Ahora |
|---|---|---|
| `obtener-estado-agente` | OK 2406 ms | **OK 98 ms** |
| `obtener-inventario` | **FALLO 5020 ms** | RECHAZO 404 ms |
| `obtener-estado-boveda` | RECHAZO 551 ms | RECHAZO **2 ms** |
| `consultar-sandbox` | **FALLO 5006 ms** | RECHAZO 408 ms |
| `consultar-alertas` | OK 346 ms | OK 435 ms |
| `obtener-condiciones` | **FALLO 5010 ms** | OK 499 ms |

**Seis de seis responden. Cero vencimientos.** Los tres `RECHAZO` son los canales que el
contrato declara `servido = false` (RPT-081): responden, y dicen por qué no tienen dato.

### 4.1 La primera vuelta, observada

Segunda corrida, con el guion lanzado a los dos segundos del arranque:

```
OK       obtener-estado-agente  (267 ms)
OK       consultar-alertas      (438 ms)
RECHAZO  obtener-condiciones    (376 ms)
         motivo: el sensor aun no ha completado su primera vuelta:
                 no hay condiciones evaluadas todavia
```

El motivo llegó literal hasta la consola, y las alertas ya persistidas se sirvieron
igual. La selectividad del §3 no es una intención escrita en un comentario: se ve.

Con esto **PA-141 nace y muere en el mismo reporte**, que es la forma correcta de abrir
un punto cuando la observación que le falta cabe en una corrida más.

## 5. Lo que cambió de verdad no es el número: es que ahora hay un techo

Antes: `latencia ≈ --tramas ÷ ritmo de tramas`. **Sin cota.** Con 200 tramas y un goteo,
once segundos; con menos goteo, más.

Ahora: `latencia ≤ min(intervalo entre tramas, PLAZO)`, y `PLAZO` son 500 ms. La consulta
espera a que llegue la siguiente trama o a que venza el plazo de silencio, lo que ocurra
primero. **Cualquiera de los dos está acotado**, y ese es el arreglo — no los
milisegundos concretos.

Los 2 ms de `obtener-estado-boveda` y los 499 de `obtener-condiciones` son las dos puntas
del mismo intervalo: una consulta que cayó justo al lado de una llamada a `atender` y
otra que cayó justo después.

## 6. Cuarta predicción de constante fallada, y la peor de las cuatro

Escrito antes de ejecutar: *«el techo lo pone el intervalo del ping, no la vuelta»* —
correcto—, y en la frase siguiente *«todas por debajo de 400 ms»*.

Con un `ping -i 0.4`, el intervalo **es** 400 ms. La espera llega hasta 400, no se queda
debajo. **El mecanismo correcto estaba escrito en el mismo párrafo que el número que lo
contradecía.**

Las tres anteriores (RPT-083 §6.2) se explicaban por estimar de cabeza en lugar de medir.
Ésta no: aquí la medición ya estaba hecha y aun así el número no se derivó de ella. La
regla que queda es más estrecha que «medir primero»: **el número se calcula a partir del
mecanismo escrito, o no se escribe.**

Predicciones de mecanismo: correctas otra vez. De constante: 0 de 4.

## 7. El camino hasta la corrida, que costó más que el arreglo

Siete intentos fallidos antes de una corrida válida. Ninguno tocó el código; todos fueron
el puente entre el anfitrión y la VM. Se anota porque el tiempo se fue ahí:

| Qué falló | Causa |
|---|---|
| `scp` rechazado | El reenvío NAT no sobrevivió al reinicio de la VM |
| Órdenes en la ventana equivocada | Los bloques no decían en qué máquina van. **Error de redacción mío** |
| `~/eje-vision` no existe | Ruta inventada; es `~/Eje-Latam/apps/eje-vision`. **Error mío** |
| `VBoxManage.exe: command not found` | No está en el `PATH` de WSL |
| `Could not find a registered machine named 'eje-prueba'` | La VM se llama `eje-prueba-pa117`; `eje-prueba` es el nombre de máquina de dentro |
| `Connection refused` con la regla NAT existente | **WSL2 no comparte el `127.0.0.1` de Windows.** Se resuelve usando `scp.exe`, que es un proceso Windows |
| `Permission denied` con el md5 correcto | El `scp` de Windows no preserva el bit de ejecución |

### 7.1 Las cuatro corridas descartadas se descartaron solas

Ninguna se leyó como «el arreglo no funciona». La consola distingue **`[sin-socket]`**,
**`[sin-escucha]`** y **`[sin-respuesta]`**, y en las cuatro dijo `[sin-escucha]`: *el
fichero está, no hay nadie detrás*. Es la doctrina de los tres estados devolviendo, en
herramienta de diagnóstico, el tiempo que costó implementarla.

Un cliente que sólo supiera «no respondió» habría producido cuatro falsos negativos sobre
un arreglo correcto.

### 7.2 El intento que el agente rechazó, y bien

Un arranque falló con:

```
con configuracion firmada, '--interfaz' no se pasa por la linea de ordenes.
```

No es un estorbo: es PA-79 funcionando en máquina real por primera vez fuera de las
pruebas. La VM tiene configuración firmada, `--interfaz` es una bandera dictada, y el
agente se niega **y dice dónde se cambia**. El binario también confirmó la comprobación
de identidad: md5 `4e959cf…` en los dos lados antes de observar nada.

### 7.3 Y el tablero cazó la fila que yo mismo escribí mal

Al cerrar PA-136 puse el marcador `🟢`. El tablero reconoce cuatro: `✅`, `🔵`, `🟡`, `🔴`.
`leer` descarta la fila entera cuando no reconoce ninguno, así que **la fila seguía en el
documento y había salido del recuento**.

`cargo xtask tablero` lo dijo sin ambigüedad: *«1 identificador citado en docs/ y SIN fila
en el tablero: PA-136»*. Es exactamente el defecto que ese comando existe para impedir —un
punto que sólo vive en el reporte que lo acunó— y esta vez el que lo cometió fue quien
escribe los reportes.

Lo que hace útil el episodio: el fallo **no era ruidoso**. El documento se veía bien, la
fila estaba donde debía y decía lo correcto. Sin la comprobación, PA-136 habría quedado
cerrado en la prosa y ausente de todo recuento a partir de hoy.

## 8. Dos hallazgos de regalo

**`consultar-alertas` devolvió 1 038 208 bytes con `hayMas: true`.** Un megabyte en una
sola respuesta que la consola todavía no sabe paginar. Es PA-97, que hasta hoy era una
nota y ahora tiene una cifra.

**`obtener-condiciones` respondió a 499 ms, a un milisegundo de `PLAZO`.** No es un
problema hoy, pero marca dónde está el borde: si `PLAZO` subiera, la latencia subiría con
él. Queda anotado como PA-140.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| PA-136 | **Cerrado.** §4 |
| PA-97 | Sigue abierto, ahora con la cifra de §8: 1 038 208 bytes y `hayMas: true` |
| PA-140 | **Nuevo.** La latencia de consulta queda atada a `PLAZO`; si `PLAZO` cambia, cambia con él y nadie lo notaría |
| PA-141 | **Abierto y cerrado hoy.** §4.1: observado con el guion lanzado a los 2 s |
| PA-78 | La mitad B sigue esperando un escritorio |

---

*Reporte Nº 84 — El arreglo de cuatro líneas · PremosCorp · 28 de agosto de 2026*
