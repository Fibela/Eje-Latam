# RPT-098 — El defecto que no estaba

**Tema:** PA-146b fase 1. `sucesora` deja de tirarse — y un hallazgo que anuncié, documenté y tuve que retirar
**Nº de reporte:** 098
**Fecha:** 12 de septiembre de 2026
**Área designada:** Método
**Entidad:** PremosCorp
**Estado:** Fase 1 cerrada. `guardian-cc` 179. **PA-151 retirado: no existía**

- **Depende de:** RPT-015 (el certificado), RPT-016 (el archivo de revocaciones), RPT-088 §4.1 (variantes para casos imposibles), RPT-097 (la prueba de la prueba)
- **Aborda:** PA-146b (parcial). **Retira PA-151**

---

## 1. Lo que sí se hizo

`CertificadoRevocacion` lleva `sucesora` desde RPT-015: verificada, persistida, con accesor. Y
`ArchivoRevocaciones::registro` la **tiraba** al derivar el registro que el agente usa.

Ahora `RegistroRevocaciones` la conserva y `sucesora_de` la devuelve. Es el sustrato de la
fase 3, donde un certificado verificado podrá por fin adoptar a la clave que nombra.

Dos estados: `SinRevocar` y `Declarada`. Enumerado y no `Option` porque `SinRevocar` dice algo
que `None` no dice, y quien lo consuma tiene que distinguir «esta clave está bien» de «esta
clave se revocó».

## 2. El hallazgo que anuncié y era falso

Al escribir el comentario que explicaba por qué ese bucle tenía que cambiar, creí ver algo más
grande y lo dije con todas las letras:

> *«`RegistroRevocaciones::anotar` conserva el corte más bajo —documentado y con pruebas— pero
> sólo se llama desde pruebas. El único camino del agente empuja pares sin fundir: dos
> certificados dejan dos entradas y `corte_de` devuelve la primera. Un certificado correctivo
> con el corte más alto afloja la revocación si cae primero.»*

Abrí **PA-151**, lo escribí en el tablero, y lo llamé «peor que el defecto que venía a
arreglar».

**Es falso, y en todas sus partes.** La invariante está protegida en dos sitios que no leí:

| Barrera | Qué impide |
|---|---|
| `ArchivoRevocaciones::anotar` | Funde por clave revocada conservando el corte más bajo — el archivo nunca lleva dos |
| `ArchivoRevocaciones::analizar` | Exige identificadores **estrictamente crecientes** — un fichero con duplicado se rechaza |

El bucle que acusé no podía recibir dos anotaciones de la misma clave. El daño que le atribuí
no era alcanzable por ninguna vía, ni siquiera escribiendo el fichero a mano.

## 3. Lo cazó la perturbación, no yo

Siguiendo lo de RPT-097, se desactivó la fusión para ver la prueba ponerse roja. Cayeron dos
pruebas — **y ninguna era la mía**:

```
test dos_sucesoras_distintas_para_la_misma_clave_quedan_en_conflicto ... FAILED
test el_registro_conserva_el_corte_mas_bajo ... FAILED
```

`desde_disco_dos_certificados_se_funden_y_gana_el_corte_mas_bajo` —la que escribí para
demostrar PA-151— **pasó en verde con la fusión rota**. No reaccionaba a que se rompiera lo
que decía comprobar, porque el archivo ya había colapsado las dos anotaciones en una capa más
arriba, antes de que el registro las viera.

Una prueba que no se cae cuando rompes aquello que afirma verificar no está verificándolo.

Sin ese paso, esto se habría commiteado con un identificador inventado, un reporte que
documentaba un ataque imposible, y una prueba vacua que lo «respaldaba».

## 4. Y el arreglo arrastraba una variante imposible

`Sucesion` tenía tres estados: el tercero, `EnConflicto`, para dos certificados que nombraran
sucesoras distintas para la misma clave. Con la invariante real a la vista, ese caso **no
puede ocurrir**.

RPT-088 §4.1 ya había decidido esto para otra cosa: una variante para un caso imposible invita
a rellenarla. Se retira, y con ella `fundir` y `anotar_bruto`, que eran el arreglo de un
defecto inexistente.

En su lugar queda `un_archivo_no_admite_dos_certificados_para_la_misma_clave`, que comprueba
las **dos** barreras y dice en su documentación qué significa que se ponga roja: que el
conflicto pasó a ser alcanzable y que alguien tiene que decidir qué hace el agente con dos
órdenes de sucesión incompatibles.

Es la diferencia entre defender un caso imposible y **anclar por qué es imposible**.

## 5. La tercera vez esta semana

| Cuándo | Qué diagnostiqué | Qué pasaba |
|---|---|---|
| 10-sep | «No existe barrera contra filas duplicadas del tablero» (RPT-094 §7) | `identificador_de` ya las rechazaba. Lleva errata |
| 10-sep | «La cabecera cambiará de tono porque `hay_manipulacion()` la incluye» | Ese método es de Rust y la cabecera de TypeScript |
| 12-sep | «La regla del corte más bajo no corre en producción» | Corre, una capa más arriba |

La forma es siempre la misma: **leo una función, veo lo que le falta, y concluyo sin leer
quién la llama.** Las tres veces el repositorio me corrigió en minutos, y las tres por un
mecanismo distinto —una regla vieja, una captura de pantalla, una perturbación deliberada—.

Lo que no tengo es una forma de cazarlo *antes* de anunciarlo. La regla que saco, y que es
barata: **antes de abrir un identificador por un mecanismo que parece no correr, leer a sus
llamantes hacia arriba hasta encontrar quien sí lo garantiza — o confirmar que no hay nadie.**
Abrir el identificador es la parte que no cuesta nada y que hace daño.

## 6. Puntos abiertos

| ID | Punto |
|---|---|
| PA-146b | **Parcial.** Fase 1 hecha; faltan el centinela v4 y la máquina de estados |
| ~~PA-151~~ | **Retirado.** No existía. §2 |
| PA-145, PA-147, PA-148 | Sin cambios |
| PA-142 | `diagnostico.js` sigue ciego |

---

*Reporte Nº 98 — El defecto que no estaba · PremosCorp · 12 de septiembre de 2026*
