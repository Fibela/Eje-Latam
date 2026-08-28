# RPT-083 — La ventana sin techo

**Tema:** PA-136. La latencia de la consola es la ventana de observación, y la ventana no tiene techo
**Nº de reporte:** 083
**Fecha:** 26 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Medido en máquina real. Sin arreglar.** El arreglo se decide con estos números encima

- **Depende de:** RPT-079 §11.2 (donde apareció), RPT-034 §4 (atender sobre lo persistido), RPT-072 (`--ciclos` como frontera)
- **Aborda:** PA-136

---

## 1. Lo que se sabía y lo que se suponía

De la VM salieron latencias de 444 a 983 ms sobre un socket de dominio Unix local,
donde deberían ser de un dígito. La explicación **parecía** obvia: el agente atiende
al final del ciclo y cada ciclo espera hasta `PLAZO` = 500 ms por trama.

Eso era aritmética, no observación. Se instrumentaron los cinco tramos de la vuelta
antes de proponer nada.

## 2. El instrumento que ya existía medía mal

Al colocar la primera marca apareció que `inicio.elapsed()` se evaluaba **después**
de `ciclo.vuelta`. El `Tramas observadas: N en X` que se imprime desde RPT-020 nunca
fue la ventana de captura: era la ventana **más** clasificar, persistir y emitir.

Las cifras que traíamos de la VM —507, 977 y 526 ms— no medían lo que creíamos.

Y lo que hace esto grave: **la predicción escrita era «la ventana de captura
domina»**, y el instrumento con el que iba a comprobarse estaba sesgado exactamente
en esa dirección. Habría leído como confirmación algo que el propio instrumento
fabricaba.

Sólo apareció al ir a colocar una marca al lado. Es la familia de toda la semana,
esta vez en la herramienta de medir.

## 3. Los cinco tramos, medidos

Con `colector = ""`, red en silencio, sobre la VM:

| Tramo | Típico | Parte |
|---|---|---|
| **captura** | 500–530 ms | **99 %** |
| estadísticas | 40–425 **ns** | 0 % |
| vuelta | 0,8–3 µs (54 µs con 12 tramas) | 0 % |
| presentar | 20–900 µs | 0 % |
| atender | 12–312 µs | 0 % |

**La predicción acertó y por mucho:** dije que los cuatro no-captura sumarían menos
de 50 ms. Suman menos de **uno**. Clasificar, persistir y atender son ruido.

## 4. La ventana no está acotada por `PLAZO`

Tres vueltas duraron 1,66 s, 1,33 s y 1,82 s con un `PLAZO` de 500 ms. La segunda
predicción —que la dispersión era alineación de fase— **falló**, y el fallo es el
hallazgo.

El bucle sigue mientras lleguen tramas. Sólo sale cuando pasan 500 ms **de
silencio** o cuando se llenan las `--tramas`. Es decir:

```
duración del ciclo  =  --tramas  ÷  ritmo de tramas
```

Sin techo, salvo el que ponga el número de tramas.

## 5. El caso patológico no es el segmento saturado: es el goteo

Con mucho tráfico, `--tramas` se llena rápido y la vuelta es corta. El caso malo es
el intermedio: tráfico suficiente para reiniciar el temporizador y no para llenar el
cupo.

Medido con `ping -i 0.4` sobre `lo`:

| `--tramas` | Vueltas observadas |
|---|---|
| 40 | 4,07 s · 4,13 s · 4,08 s |
| **200** (el valor por omisión) | **10,70 s** y **12,88 s** |

**La unidad desplegada no pasa `--tramas`**, así que usa 200. Y un goteo constante
es exactamente el tráfico de un segmento OT: un PLC sondeando.

## 6. La demostración, sin aritmética de por medio

Agente en modo servicio, goteo de fondo, y la consola preguntando:

```
OK       obtener-estado-agente   2406 ms
FALLO    obtener-inventario      5020 ms  [sin-respuesta]
RECHAZO  obtener-estado-boveda    551 ms
FALLO    consultar-sandbox       5006 ms  [sin-respuesta]
OK       consultar-alertas        346 ms
FALLO    obtener-condiciones     5010 ms  [sin-respuesta]
```

**Tres de seis vencen el plazo de 5 s.** Los que responden lo hacen a 2406, 551 y
346 ms: los que cayeron cerca del final de una vuelta.

Con la configuración que se despliega, un goteo de tráfico y **ninguna avería en
ninguna parte**, la mitad de la consola no funciona.

Y matiza el resultado de PA-78: la conversación de anteayer funcionó porque `lo`
estaba en silencio.

### 6.1 Una corrida que no contó

Un intento anterior devolvió seis fallos con `[sin-socket]` y **no se contó**. El
agente había terminado sus dos vueltas y salido antes de lanzar el guion; eso no es
la consola venciendo, es que no había nadie. `enlace.ts` mantiene esas dos causas
separadas desde RPT-046 precisamente para esto.

### 6.2 Tres predicciones de constante, tres fallos

El mecanismo se predijo bien las tres veces. El número, ninguna: 16 s contra 4,08;
20 s contra 10,7; y antes, la ventana de 500 ms.

La causa es siempre la misma: estimar el ritmo de tramas de `lo` de cabeza en lugar
de medirlo. Queda medido —200 tramas en 10,7 s, unas 18,7/s— y de ahí sale a partir
de ahora.

Se anota porque una constante mal estimada decide si algo es un inconveniente o una
catástrofe, y aquí decidía justo eso.

## 7. El arreglo, que resultó ser pequeño

Con los números delante desaparecen las tres soluciones que se barajaron ayer. No
hace falta un hilo aparte, ni tocar la ventana de observación, ni acortar `PLAZO`.

`atender` cuesta **microsegundos**. Llamarlo dentro del bucle de captura, entre
trama y trama, son cuatro milisegundos sobre una vuelta de once segundos.

### 7.1 Y no es una concesión

Parecía un intercambio —latencia contra frescura— y no lo es.

Atender a mitad de vuelta sirve el estado de la **vuelta anterior**, que ya está
persistido: cumple RPT-034 §4 sin tocarlo. Atender al final sirve el estado fresco,
pero **llega hasta once segundos tarde**.

**La edad del dato que recibe el operador es la misma en los dos casos.** La
diferencia es que uno responde y el otro se cuelga.

### 7.2 La única pregunta que queda

En la primera vuelta todavía no hay condiciones calculadas. Servir «no hay» sería
mentir; hay que **rechazar con motivo**, que es la doctrina de los tres estados en un
sitio nuevo.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| PA-136 | Medido y demostrado (§6). **Sin arreglar**: el arreglo es §7 y se decide con esto encima |
| PA-78 | El resultado de la mitad A se matiza: se midió con la red en silencio (§6) |

---

*Reporte Nº 83 — La ventana sin techo · PremosCorp · 26 de agosto de 2026*
