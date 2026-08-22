# RPT-055 — La décima condición: un sensor que no informa a nadie

**Tema:** PA-109. `sinColector` en los seis sitios, y la segunda excepción a la emisión
**Nº de reporte:** 055
**Fecha:** 13 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado.** Cierra PA-109

- **Depende de:** RPT-054 §4 y §5 (dónde puede declararse), RPT-053 (`Latido::SinColector`), RPT-019 §2 (qué es una condición)
- **Aborda:** PA-109

---

## 1. El hueco

RPT-053 introdujo `Latido::SinColector` y lo imprimió por pantalla. Eso llega a
`journald` y a ningún sitio más: **VIS-04 lee `Condiciones` por IPC**, y ahí no
estaba.

El técnico que va a la planta a averiguar por qué un sensor no aparece en la sala
tenía que saltar del tablero a los diarios del sistema para enterarse. Un tablero
que obliga a mirar otra cosa para entender lo que muestra no es un tablero.

## 2. Por qué es una condición y no un dato de configuración

Porque es **verdadera hasta que alguien interviene**, que es la definición literal
de RPT-019 §2. No ocurre: es. No se resuelve esperando: se configura.

Encaja mejor que casi ninguna de las otras nueve.

## 3. Lo que la distingue de `salidaNoDisponible`

Las dos dicen que la alerta no sale del equipo, y mandan al técnico a sitios
distintos:

| | `salidaNoDisponible` | `sinColector` |
|---|---|---|
| Qué pasó | El colector existe y no responde | Nunca hubo colector |
| Se resuelve | Sola, cuando la red vuelve | Configurando |
| A quién se llama | A quien mantiene el SIEM | A nadie: se instala bien |

Colapsarlas mandaría al operador a investigar una caída de red inexistente, y al
revés haría pasar una caída real por una instalación a medias.

De ahí que en la cabecera de VIS-04 `sinColector` vaya **después**: una avería en
curso es más urgente que una instalación incompleta. Hay prueba del orden. No
pueden estar activas a la vez —sin colector configurado no hay envío que falle—
pero el orden se fija igualmente: la próxima persona que añada una rama no tiene
por qué saber eso.

## 4. La segunda condición no emisible

Un agente sin colector **no puede avisar de que no tiene colector**: el aviso
viajaría por el canal que no existe. Es la misma imposibilidad que mantiene
`salidaNoDisponible` fuera de syslog (RPT-032 §4).

`EMISIBLES` pasa a ocho de diez. La barrera de PA-91 —la que obliga a que toda
condición nueva salga al SIEM o se declare excepción— dejó de tener una excepción
escrita en un `if` y ahora las nombra en una lista:

```rust
const NO_EMISIBLES: [&str; 2] = ["salidaNoDisponible", "sinColector"];
```

La prueba entera se deriva de esa lista, incluido el recuento. Una tercera
excepción futura tendrá que escribirse ahí, a propósito y con su motivo al lado:
**la barrera protege contra el olvido, no contra la decisión.**

## 5. Cuenta como degradación

`hay_degradacion()` la incluye. Un sensor sin colector cumple su trabajo local y
**no cumple la promesa del producto**: que la alerta salga del equipo.

Que sea deliberado no lo hace menos cierto, y ocultarlo por deliberado es
exactamente como se despliega una flota entera sin vigilar (RPT-054 §1).

Silenciarlo cuando la ausencia de colector sea una decisión **declarada** es cosa
de la configuración firmada, no de la condición. Es PA-79.

## 6. Una sola fuente

El valor sale de `self.emisor.is_none()`, la misma de la que sale
`Latido::SinColector`. Un interruptor aparte habría permitido que el tablero
dijera que hay colector y el latido que no, y esa contradicción no la habría
notado ninguna prueba: las dos habrían pasado por separado.

Entra en `condiciones()` como **parámetro**, no rellenándose después. Es la
lección de RPT-047 §4: un campo que se fija después es un campo que alguien
olvidará fijar, y aquí el olvido se lee como «hay colector».

## 7. Los seis sitios

| Sitio | Qué cambió |
|---|---|
| `contrato-ipc.toml` | Campo con el motivo de por qué no se emite |
| `eje-ipc/mensajes.rs` | Campo, `CAMPOS_CONDICIONES` a 10, `hay_degradacion` |
| `salida.rs` | Excluida de `EMISIBLES` y de `valor_de`, con la lista del §4 |
| `puente.ts` | Interfaz y `CAMPOS_CONDICIONES` del lado TypeScript |
| VIS-04 | Fila en la tabla, rama en la cabecera, panel de diagnóstico |
| Pruebas de paridad | Las de ambos lados, que son las que impiden quedarse a medias |

Verificado: workspace de Rust en verde con `-D warnings`, y 114 pruebas de
`eje-vision` en 18 suites, sin fallos.

## 8. Una advertencia de nombre

`verificarPaquete` **ya existe** en `eje-vision`, y verifica el paquete del módulo
empresarial con su firma y su licencia (RPT-003 §3.4). No tiene nada que ver con
la verificación del artefacto instalado que RPT-054 §3.1 declara imposible desde
dentro del proceso.

PA-111 no debe reutilizar ese nombre. Dos comprobaciones distintas con el mismo
nombre es cómo alguien concluye que la segunda ya está hecha.

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-109~~ | ✅ Implementado en los seis sitios |
| PA-79 | Declarar la ausencia de colector como decisión, para poder silenciarla sin ocultarla (§5) |
| ~~PA-106~~ | ✅ Cerrado en RPT-056: paridad **declarada**, no igualdad — las dos no emisibles no viajan por syslog a propósito |
| PA-111 | Verificación del artefacto desde fuera del proceso. **No** llamarla `verificarPaquete` (§8) |
| ~~PA-104~~ | ✅ Cerrado por observación en RPT-057 §4 |

---

*Reporte Nº 55 — La décima condición · PremosCorp · 13 de agosto de 2026*
