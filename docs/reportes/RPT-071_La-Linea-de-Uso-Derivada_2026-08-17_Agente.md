# RPT-071 — La línea de uso derivada

**Tema:** PA-122. La ayuda y el analizador eran dos listas escritas a mano
**Nº de reporte:** 071
**Fecha:** 17 de agosto de 2026
**Área designada:** Agente
**Entidad:** PremosCorp
**Estado:** Construido y probado. Cierra PA-122

- **Depende de:** RPT-066 (la misma solución para `xtask`), RPT-069 §2 (el hallazgo), RPT-060, RPT-039 §8
- **Aborda:** PA-122

---

## 1. El hallazgo salió de la comprobación más barata del protocolo

`./eje-agente` sin argumentos existía para descartar incompatibilidad de
bibliotecas en la máquina de pruebas. Tardó dos segundos y dijo otra cosa:

```
uso: eje-agente --interfaz <nombre> [--tramas <n>] … [--nombre <maquina>]
```

Sin `--directorio-socket`, que se había añadido el día anterior. Una opción que
existe, funciona y **nadie puede descubrir**.

## 2. El quinto índice de la semana

| Índice | Lector que lo deriva |
|---|---|
| Puntos abiertos | `cargo xtask tablero` (PA-108) |
| Pruebas escritas | `cargo xtask cobertura` (PA-73) |
| Comandos del manual | `cargo xtask manual` (PA-119) |
| Órdenes de `xtask` | la tabla `ORDENES` (RPT-066) |
| **Opciones del agente** | **esto** |

Cinco veces el mismo patrón en una semana. Lo general no es ninguno de ellos:
**todo índice escrito a mano de cosas que viven en el código necesita un lector
que lo derive**, o se queda atrás y sigue pareciendo completo.

## 3. Una dirección por construcción, la otra por prueba

`OPCIONES` es la fuente única. De ahí sale la línea de uso, y **la puerta está
antes del `match`**:

```rust
if !OPCIONES.iter().any(|opcion| opcion.bandera == clave) {
    return Err(ErrorAgente::Uso);
}
```

Con eso, aceptar una bandera que la ayuda no anuncie es **imposible**, no
improbable.

La dirección contraria —anunciar una que el `match` ignore— no se puede cerrar
igual, porque un `match` no se enumera sin leer el fuente, y este proyecto ya
aprendió dos veces lo que cuesta leer fuente sin lexer. Se cubre con una prueba
que ejercita **cada entrada de la tabla** a través del analizador de verdad,
usando el valor de ejemplo que la propia entrada trae.

## 4. El campo que sólo existe en las pruebas

El valor de ejemplo lo usa la prueba y nadie más. El compilador lo dijo:

```
error: field `ejemplo` is never read
```

Las dos salidas fáciles eran peores que el aviso:

- `#[allow(dead_code)]` apaga un instrumento que decía la verdad. Ya se rechazó
  una vez, en RPT-062.
- Una tabla de ejemplos dentro del módulo de pruebas sería un **sexto** índice a
  mano, dentro justo de la barrera que existe para cazar esos.

El campo lleva `#[cfg(test)]`: se retira del binario en lugar de callar el aviso.
El dato es de prueba y ahora sólo existe en compilaciones de prueba.

## 5. Y una prueba que declaraba un propósito que no cumplía

`tests/uso.rs` traía una lista de cuatro banderas con este comentario:

> *«No están todas a propósito: se listan las que un operador necesita para
> arrancar el agente y las que se añadieron tarde, que son las que se olvidan.»*

`--directorio-socket` se añadió tarde, se olvidó, y **no estaba en la lista**. La
prueba pasó en verde el día entero que la opción fue indescubrible.

No es un descuido de quien la escribió: es que **una lista parcial no puede
cumplir esa promesa**, porque «las que se añaden tarde» son, por definición, las
que aún no están en ella.

La suite se quedó con lo que sí es suyo y no se puede comprobar de otro modo: que
el mensaje **salga del binario de verdad** y llegue a `stderr`. Ese era su
propósito original (PA-85), y para eso sobra con una bandera. La completitud la
lleva ahora la prueba unitaria, que sale de la misma tabla que la línea.

## 6. Lo que queda sin cubrir

Que el **texto** de cada opción describa lo que hace. `OPCIONES` lleva la
bandera, la forma del valor y si es obligatoria; no lleva una frase de ayuda,
porque el mensaje de uso de una línea no tiene sitio para diez. Un `--ayuda` con
descripciones es una decisión de producto y no se coló aquí.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-122~~ | ✅ **Cerrado** (§3) |
| PA-79 | Media tabla de opciones desaparece cuando la configuración sea firmada |
| PA-84 | `--grupo-ipc` aceptaría un nombre de grupo y no un número |

---

*Reporte Nº 71 — La línea de uso derivada · PremosCorp · 17 de agosto de 2026*
