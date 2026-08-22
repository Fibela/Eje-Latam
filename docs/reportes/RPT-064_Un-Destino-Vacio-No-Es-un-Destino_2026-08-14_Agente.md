# RPT-064 — Un destino vacío no es un destino

**Tema:** PA-118. La unidad de `systemd` anulaba la décima condición
**Nº de reporte:** 064
**Fecha:** 14 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** **Corregido y verificado por observación.** Cierra PA-118

- **Depende de:** RPT-055 §3 (por qué `sinColector` y `salidaNoDisponible` son distintas), RPT-062 (la unidad), RPT-054 §1 (instalar sin colector es legítimo)
- **Aborda:** PA-118

---

## 1. Lo que encontró la Fase 1

La unidad se instaló en un `systemd` real, con `EJE_COLECTOR` vacío — el
despliegue que RPT-054 §1 ratificó como legítimo y del que el propio instalador
presume en su aviso.

```
argv[]=/usr/local/bin/eje-agente --interfaz ${EJE_INTERFAZ} ... --syslog ${EJE_COLECTOR} ...

Salida de alertas  :
salidaNoDisponible : true      <- averia
sinColector        : false     <- MENTIRA
```

**La unidad convertía «este sensor no informa a nadie» en «el colector de este
sensor está caído».** Son las dos cosas que RPT-055 §3 separó a propósito, y
mandan al técnico a sitios distintos: una a llamar a quien mantiene el SIEM, otra
a terminar la instalación.

La décima condición, la que se construyó hace un día entero para distinguir
exactamente eso, anulada por un fichero de configuración.

## 2. Por qué `systemd` no puede arreglarlo

`${VARIABLE}` se sustituye como **un argumento, vacío incluido**. `$VARIABLE`, en
cambio, se parte en palabras — y por eso no sirve: una ruta con espacios se
convertiría en dos argumentos.

Y no hay condicionales dentro de `ExecStart`. La unidad **no puede** omitir el
par `--syslog <valor>` según el contenido de una variable.

Predije que `systemd` dejaría `--syslog` suelto y que el agente saldría con error
de uso. **Me equivoqué**, y el error real era peor: no rompe, miente.

## 3. La corrección va en la frontera del agente

```rust
pub fn colector_declarado(valor: &str) -> Option<&str> {
    let limpio = valor.trim();
    if limpio.is_empty() { None } else { Some(limpio) }
}
```

No es un apaño para `systemd`. **`Some("")` nunca fue un estado legítimo**:
`"".to_socket_addrs()` no puede resolver jamás, así que era un valor que el tipo
permitía y el dominio no. Eliminarlo en la frontera es lo mismo que se hizo con
`Latido` (RPT-053 §2) y con `Identidad` (RPT-059): quitar del tipo un estado que
no existe.

Y no se sustituye en silencio. El agente lo declara en el arranque:

```
Salida de alertas  : NINGUNA; las alertas no salen del equipo y nadie fuera notara si se apaga
```

## 4. La observación que cierra PA-118

Mismo montaje, con el artefacto reconstruido:

```
salidaNoDisponible : false
sinColector        : true
```

Las dos invertidas respecto al §1. La predicción se escribió antes de tocar el
código.

## 5. La Fase 1 hizo de linter, y por eso valió

Se acordó que WSL con `systemd` sería un **linter de ejecución** y que sólo una
máquina limpia cerraría PA-117. Se dijo antes de ejecutar, y se cumplió: la Fase 1
no confirmó la unidad, **encontró que mentía**.

Es la segunda vez en dos días que una comprobación descubre un comportamiento
ausente en lugar de ratificar uno presente — la primera fue el aviso del colector
en RPT-063 §2. Las dos aparecieron al ejercitar el caso que la documentación
declaraba legítimo y que nadie había ejecutado.

## 6. Tres veces evidencia del proceso equivocado, en media hora

Esto merece quedar escrito porque estuvo a punto de costar dos conclusiones
falsas:

- Se leyeron líneas del diario del **PID 4397** —el arranque *con* colector— como
  si fueran del 4518. Con un colector configurado que no responde,
  `salidaNoDisponible: true` con `sinColector: false` **es la respuesta
  correcta**, así que la evidencia parecía confirmar el defecto y no lo mostraba.
- Se leyó una observación tomada con el **artefacto sin regenerar**: `empaquetar`
  había fallado por un error de sintaxis y `instalar.sh` copió el binario
  anterior. La corrección estaba compilada y no instalada.
- `systemctl show -p ExecStart` salió **truncado por el paginador**, y la línea
  cortada era la única que decidía.

Las tres se evitan igual: **encadenar con `&&`** para que un fallo detenga lo que
viene detrás, y comprobar el PID antes de leer un diario. Un montaje que sigue
adelante tras un fallo produce salidas que parecen resultados.

## 7. Un error de bulto, para el registro

El comentario que explica todo esto se escribió **dentro de un literal de cadena
de Rust** y contenía comillas dobles. Cerraron el literal y la unidad entera se
convirtió en código: veinte líneas de fichero `.service` interpretadas como Rust.

Lo cazó el compilador de inmediato. Se anota porque el patrón —texto de
configuración incrustado en el código fuente— seguirá estando ahí, y con él el
riesgo.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-118~~ | ✅ **Cerrado por observación** (§4) |
| PA-117 | La comprobación 4. Sigue exigiendo máquina limpia |
| PA-79 | La configuración sigue siendo un fichero de texto editable |

---

*Reporte Nº 64 — Un destino vacío no es un destino · PremosCorp · 14 de agosto de 2026*
