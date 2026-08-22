# RPT-072 — El agente se calla

**Tema:** PA-123. El informe completo cada vuelta inundaba `journald`
**Nº de reporte:** 072
**Fecha:** 17 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Cerrado por observación en un servicio real.** Cierra PA-123

- **Depende de:** RPT-069 §2 (el hallazgo), RPT-052 §1 (el silencio no es una afirmación), RPT-034 (el modo continuo), RPT-058 §PA-114
- **Aborda:** PA-123

---

## 1. Lo que se observó

En la máquina de pruebas, con el sensor instalado y un segmento sin tráfico:

```
Aug 15 02:17:19 … Condiciones vigentes: …
Aug 15 02:17:20 … Condiciones vigentes: …
```

Dos informes completos **entre dos segundos consecutivos**. Unas 50 líneas por
segundo, indefinidamente, diciendo que no pasa nada.

El coste no es el disco: es que **el diario rota tan rápido que borra lo que
importaba**. Cuando ocurra un incidente y alguien mire qué pasó hace dos horas,
no habrá nada que mirar. Un registro que se sobrescribe a sí mismo es peor que
uno que no existe, porque parece que existe.

## 2. La misma familia de siempre

El informe completo es **presentación para una persona delante de un terminal**.
El modo demonio lo ejecutaba dos veces por segundo.

Es el reloj congelado de RPT-036 §3 y la reemisión del historial de RPT-037 §3
otra vez: **código correcto escrito para ejecutarse una vez, ejecutándose
muchas**. Van cuatro.

## 3. La distinción ya existía y no se añadió una bandera

`--ciclos 0` es el servicio; `--ciclos N` finito es el recorrido de comprobación.
La voz se deriva de ahí:

```rust
const fn voz_de(ciclos: u64) -> Voz
```

**No se añadió `--verboso` ni nada parecido.** Dos formas de decir lo mismo se
contradicen el día que alguien cambie una, y ya hay una opción que dice
exactamente esto.

## 4. Qué se dice y qué se calla

| Se dice | Se calla en modo servicio |
|---|---|
| Transiciones de condiciones, encendido **y** apagado | Las once condiciones cuando ninguna se movió |
| Alertas anexadas, pérdidas, fallos de persistencia, rotación | Recuento de tramas y tabla de dispositivos |
| El latido que **no** salió | `Emitido`, `NoTocaba` y `SinColector` |
| Una señal de vida a la cadencia del latido | «sin tramas en 500 ms», que era la línea más repetida |
| — | «Consultas atendidas», que con una consola abierta se imprimía sola cada dos segundos |

## 5. Por qué hay señal de vida, y por qué no cuelga del latido

El silencio absoluto sería correcto **e inservible**: un agente atascado y uno
vigilando un segmento tranquilo dejarían el mismo diario, que es ninguno. Es
RPT-052 §1 —el argumento que obligó a inventar el latido— aplicado a `journald`
en lugar de a la sala.

```
vivo: vueltas=1240 dispositivos=3 degradado=false
```

A la cadencia del latido, unas 8 600 líneas al día frente a los 4,3 millones de
antes.

**Y no se ata a `Latido::Emitido` a propósito.** Un sensor sin colector no late
nunca, y ése es justamente el caso en que el diario local es el **único** testigo
que existe. Colgar la señal de vida del latido la habría apagado exactamente
donde más falta hace.

## 6. Las transiciones se derivan, no se listan

`condiciones_que_cambiaron` compara las dos `enumerar()` posición a posición. Una
lista escrita aquí habría sido el sexto índice a mano de la semana, y ya sabemos
cómo acaban: la de `presentar` se quedó en **siete de diez** —sin
`capturaNoDisponible`, la más grave— y nadie lo vio hasta leerlo en una consola
de verdad (PA-114).

La función devuelve datos y no imprime: así se prueba sin capturar `stdout`, que
es la misma razón por la que el ciclo vive en la biblioteca.

**La primera vuelta anuncia lo activo y calla lo apagado.** Sin ese filtro, un
sensor sano escupiría once líneas al arrancar diciendo que no pasa nada, y quien
las lea aprenderá a saltárselas — que es como se pierde la única señal que
importa.

## 7. Lo que se rechazó, y por qué

El equipo propuso además niveles de severidad con una biblioteca de registro
(`RUST_LOG=debug`) para esconder el informe sin destruirlo. El objetivo es
correcto y **el camino ya existe**: se lanza un segundo agente a mano con
`--ciclos 3` y `--directorio-socket` distinto, que es exactamente lo que se hizo
para observar PA-125 (RPT-070 §7).

Añadir `tracing` o `env_logger` metería una dependencia, un eje de configuración
nuevo y una frontera más que `cargo deny` tiene que vigilar, para abrir una
puerta que ya está abierta y probada. Si algún día hacen falta severidades en
`stdout`, `systemd` las lee del prefijo `<N>` sin biblioteca alguna.

## 8. La observación que lo cierra

Servicio instalado, reiniciado y medido:

```
21 líneas en cinco minutos   (antes: 13 788)
```

Y con la ventana ya pasado el banner de arranque, lo que queda es exactamente lo
que se diseñó:

```
19:10:33 vivo: vueltas=721  dispositivos=0 degradado=true
19:11:33 vivo: vueltas=839  dispositivos=1 degradado=true
19:12:33 vivo: vueltas=959  dispositivos=1 degradado=true
19:13:33 vivo: vueltas=1079 dispositivos=1 degradado=true
19:14:33 vivo: vueltas=1199 dispositivos=1 degradado=true
```

Una línea por minuto —la cadencia del latido por omisión son 60 s—: **1 440
líneas al día frente a 4,3 millones**, tres órdenes de magnitud.

**Y el contador de vueltas dice lo que hacía falta comprobar.** De 721 a 839 en
un minuto son **118 vueltas**, casi dos por segundo: el agente sigue girando
igual de rápido. Se calló sin perder resolución, que era la condición que el
equipo puso al descartar la opción de bajar la cadencia del ciclo.

`degradado=true` es correcto en ese equipo: no tiene colector configurado ni clave
aprovisionada.

### Por qué esta observación vale en WSL y la de PA-117 no valía

PA-117 afirmaba cosas sobre **`systemd`** —que `Restart=always` devuelve el
proceso, que `ProtectSystem=strict` confina— y eso exige `systemd` como PID 1 de
verdad. Ésta afirma algo sobre **lo que el binario escribe por su salida**, que
no depende de quién sea el PID 1. El listón no se rebaja: es que la afirmación es
de otra clase.

Lo que sí hizo falta, y las cuatro veces anteriores no: **comprobar la identidad
del artefacto antes de medir**. La primera medición dio 13 788 líneas contra el
binario del 14 de agosto, instalado tres días antes y nunca sustituido. Se cazó
por una propiedad que lo hacía imposible —el código nuevo emite `vivo:` y ahí no
había ninguna— y por `md5sum` a los dos lados.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-123~~ | ✅ **Cerrado por observación** (§8) |
| PA-126 | El formato de distribución del paquete |
| PA-79 | La cadencia del latido sigue saliendo de un argumento de línea de órdenes |

---

*Reporte Nº 72 — El agente se calla · PremosCorp · 17 de agosto de 2026*
