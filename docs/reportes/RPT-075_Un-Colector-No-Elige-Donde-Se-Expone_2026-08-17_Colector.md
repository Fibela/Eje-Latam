# RPT-075 — Un colector no elige dónde se expone

**Tema:** PA-128. El punto de escucha fijo del vigía, y el guardián que lleva días sin correr
**Nº de reporte:** 075
**Fecha:** 17 de agosto de 2026
**Área designada:** Colector
**Entidad:** PremosCorp
**Estado:** Construido y probado. Cierra PA-128

- **Depende de:** RPT-003 §9.5 (el guardián), RPT-057 (el vigía), RPT-069 §7 (el mismo descuido con VirtualBox)
- **Aborda:** PA-128

---

## 1. El hallazgo

```
FALLO — Puntos finales y credenciales de ejemplo
   crates/eje-vigia/src/main.rs:49 → let mut escucha_en = "127.0.0.1:5514".to_owned();
```

Un punto de escucha fijo en el código del colector de referencia.

## 2. Por qué no es una queja del linter

La regla existe por algo concreto: **esa cadena decide en qué interfaz escucha un
servicio de red**.

`127.0.0.1:5514` funciona en la máquina de quien lo escribió. El día que alguien
quiere que le llegue el tráfico de otro equipo, cambia el uno por un cero, y el
colector de la sala queda expuesto **a toda la red del cliente** sin que nadie
haya tomado esa decisión conscientemente.

No es hipotético: es exactamente el descuido que costó media hora hace dos días
con la regla de reenvío de puertos de VirtualBox, donde dejar la *IP anfitriona*
vacía habría publicado el SSH de la máquina de pruebas en la red local
(RPT-069 §7).

Se cierra como se cerró `EJE_INTERFAZ` en el agente: **obligando a declararlo**.
`--escuchar` deja de tener valor por omisión.

## 3. Lo que se rechazó del arreglo propuesto

Se sugirió leerlo de una variable de entorno con `.expect("…")`.

Dos motivos para no hacerlo. `expect` está **prohibido por los lints del
workspace** en código de producción. Y hace que el colector entre en pánico, que
es lo que se rechazó ayer para el agente por razones que valen igual aquí: un
proceso que muere no puede contar por qué murió.

El vigía sale con código 2 y una línea de uso que explica qué falta y por qué
importa.

## 4. Lo que de verdad falló no fue el código

Esa línea entró el **13 de agosto** con RPT-057. El guardián la habría cazado el
mismo día.

`cargo xtask verificar crates` está en la lista de comprobaciones **obligatorias**
de RPT-003 §9.4 y de `docs/Comandos.md` §3 — y no se ejecutó en cuatro días. Las
suites, `clippy` y el tablero sí; ésta no, porque no estaba en la cadena que se
teclea de memoria.

**Es la misma familia que todo lo de esta semana, un piso más arriba:** no un
índice que se queda atrás, sino una **comprobación que existe y nadie llama**.
Estaba documentada, probada y desatendida.

La secuencia completa de `docs/Comandos.md` §12 la incluye. Que exista escrita no
basta si lo que se teclea es otra cosa; por eso `verificar` entra ahora en la
misma cadena que el resto en el uso diario, y CI ya la corría en su propio job.

## 5. El guardián acusó dos veces, y sólo una tenía razón

Al quitar el valor por omisión, el guardián volvió a bloquear con tres hallazgos
nuevos: dos en mensajes de ayuda y uno en un comentario.

**Los dos de los mensajes eran correctos**, y no por el patrón. Una línea de ayuda
que reparte `una dirección concreta` es exactamente cómo el ejemplo acaba siendo
el despliegue — la decisión de RPT-054 §4.1 sobre el colector de ejemplo, que ya
dijo que *«una dirección de ejemplo aquí sería peor que ninguna»*. Los mensajes
pasan a nombrar el concepto: qué hay que decidir, no qué hay que teclear.

**El del comentario era un falso positivo**, y de la familia más conocida de esta
casa: la comprobación mira la **línea cruda**. Acusó a la prosa que explica por
qué no hay valor por omisión.

Lo llamativo es que el remedio ya existe en el mismo binario:
`xtask/src/exclusion.rs` lleva un analizador léxico completo —cadenas, cadenas
crudas, literales de carácter, comentarios de línea y de bloque— construido
precisamente porque un `grep` sobre líneas produce falsos positivos y negativos.
Esta comprobación no lo usa. Queda como **PA-129**.

**Lo que no se hizo:** ofuscar el literal para que el escáner no lo viera. Se
propuso, y habría sido peor que un `#[allow]`: editar la verdad para complacer al
instrumento. Hoy el comentario nombra el concepto porque el concepto se explica
igual de bien sin el literal; el instrumento se arregla en su punto.

## 6. Lo que este reporte **no** afirma

Que no queden más puntos finales fijos. El guardián recorre `crates/`, y esta
ejecución encontró **uno**. Lo que no sabemos es cuántos habría encontrado si se
hubiera ejecutado cada día — la respuesta es cero, porque los habría cazado al
entrar.

## 7. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-128~~ | ✅ **Cerrado** (§2) |
| PA-129 | El guardián de puntos finales mira la línea cruda, no el código (§5) |
| PA-79 | La configuración firmada, en construcción. El vigía no la usa: es la otra punta |

---

*Reporte Nº 75 — Un colector no elige dónde se expone · PremosCorp · 17 de agosto de 2026*
