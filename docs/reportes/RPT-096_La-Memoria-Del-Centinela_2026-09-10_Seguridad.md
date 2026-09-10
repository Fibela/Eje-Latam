# RPT-096 — La memoria del centinela

**Tema:** PA-150. El centinela recuerda qué clave de recuperación tuvo, y lo que eso destapó
**Nº de reporte:** 096
**Fecha:** 10 de septiembre de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** **Cerrado por observación en máquina real.** Tres capturas en `docs/evidencia/`

- **Depende de:** RPT-015 §4 (las dos claves), RPT-078 (el centinela y su doctrina), RPT-095 (PA-146a), RPT-011 §2 (el `Absorbedor`)
- **Aborda:** PA-150 (cerrado). Abre PA-149. Desbloquea PA-146b

---

## 1. Lo que el ancla compra, dicho sin adornos

El centinela pasa a versión 3 y lleva la huella de la clave de recuperación que
el sensor tuvo. Con memoria, «nunca la hubo» y «la había y ya no está» dejan de
verse igual.

**No es resistencia a `root`.** Ese fichero es de `root`, y quien borre la clave
puede borrar la marca. Lo que se compra es que el borrado deje de ser
**silencioso**, que es exactamente lo que RPT-078 §5 ya decía del centinela
entero: *«lo que se consigue es que revertir no sea silencioso»*.

Esto se escribe aquí porque el plan original lo llamaba una barrera anti-`root`,
y presentarlo así haría que alguien dejara de buscar la defensa real.

## 2. Cuatro estados, no dos

El desdoble propuesto tenía dos. Faltaba el peor.

| Ancla | Fichero | Lectura |
|---|---|---|
| ausente | ausente | **no aprovisionada** — ceremonia pendiente |
| ausente | **presente** | **no aprovisionada** — hay clave y nadie la ancló |
| presente | ausente | **suprimida** — sabotaje |
| presente | otra huella | **no verifica** — secuestro de identidad |
| presente | misma huella | anclada |

Borrar y sustituir no son lo mismo: lo primero se remedia reponiendo el fichero;
lo segundo significa que existe una clave de recuperación viva que no es la
nuestra, y con ella se firman certificados que este sensor creería. Es el mismo
par que el inventario lleva partido desde RPT-017.

### 2.1 La segunda fila es la que sostiene el diseño

Un fichero presente **sin** ancla se lee como no aprovisionada, no como anclada.
Es la negativa al «anclar al primer uso», y sin ella todo lo demás sobra: si el
agente adoptara la clave por estar ahí, el ataque completo sería *borro los dos
ficheros y dejo la mía*, y el centinela volvería a no decir nada.

Por eso el ancla la escribe `eje-manifiesto migrar-centinela`, con un humano
delante, y el agente sólo la lee.

## 3. Dos consecuencias que el plan no tenía

**`vacio()` cambió de significado.** Hasta hoy, un centinela sin ninguna de las
dos secuencias era corrupto, y el motivo era bueno: eso ya lo dice la ausencia
del fichero. Con el ancla deja de serlo — un sensor recién aprovisionado, con
clave anclada y sin inventario ni configuración, es legítimo y es **el estado más
importante que este campo existe para poder afirmar**. Con la regla anterior su
fichero se habría leído como corrupto. Tiene prueba propia, y esa prueba dice en
su documentación que si se pone roja no se ajusta: se investiga.

**La migración no es en caliente.** Las dos combinaciones cruzadas —agente viejo
con centinela v3, agente nuevo con centinela v2— dejan el sensor caído, porque
ninguna de las dos versiones tolera la otra a propósito. Binario y centinela se
cambian juntos, con el servicio parado. No es un defecto: es lo que significa
cambiar un formato de estado, y un cliente con cincuenta sensores necesita
saberlo antes y no durante.

## 4. La prueba de fuego: tres estados en tres minutos

Sobre `eje-prueba`, con el agente como servicio y VIS-04 en el escritorio.

| Captura | Cabecera |
|---|---|
| `RPT-096_Anclada` | *Este sensor no informa a ninguna sala* (el colector, como siempre) |
| `RPT-096_Sustituida` | **La clave de recuperación de este sensor no es la suya** |
| `RPT-096_Suprimida` | **Alguien borró la clave de recuperación de este sensor** |

Las tres condiciones se ven excluyentes en las dieciséis filas, y el sensor
vuelve a su estado bueno al reponer la clave legítima. La clave impostora es una
clave **legítimamente formada y del dominio correcto**: el agente no la rechaza
por malformada, la rechaza por no ser la suya.

La migración de `eje-prueba` conservó la marca de configuración en 1. Migrar
conserva; no reinicia.

## 5. Lo que encontró una captura y no encontraron 412 pruebas

Con las 176 de Rust, las 128 de TypeScript y las 105 de xtask en verde, la
primera captura de la sustitución mostraba al agente gritando *«trate este equipo
como comprometido»* mientras **la cabecera titulaba sobre el colector que falta**.

La causa: `componerCabecera` lleva su propia lista de qué condiciones son
manipulación, escrita a mano en TypeScript, y `hay_manipulacion()` lleva la suya
en Rust. Al añadir las dos de recuperación al lado de Rust, la de TypeScript se
quedó con dos. Y el comentario que las acompañaba decía que respetaba la
separación de Rust *«en lugar de reinventarla»* — **mientras la reinventaba.**

Es el décimo índice escrito a mano de la serie y el primero que cruza la frontera
de lenguaje. Se arregla el síntoma aquí, con tres pruebas nuevas; la causa —dos
listas sin nada que las coteje— queda en **PA-149**, aparte, para no taparla
dentro de este punto.

### 5.1 Y mi predicción falló por la razón correcta

Predije que la cabecera cambiaría de tono «porque `hay_manipulacion()` ahora la
incluye». `hay_manipulacion()` es de Rust y la cabecera se compone en TypeScript:
una cosa no toca la otra. Escribí de memoria sin mirar dónde vive lo que estaba
prediciendo, que es la misma falta que este reporte documenta en §5 y que llevo
cometiendo toda la semana.

## 6. Tres órdenes de shell mal dictadas, y todas con la misma forma

Se anotan porque son el mismo defecto que perseguimos en el código:

| Qué dije | Qué pasó |
|---|---|
| `scp.exe` desde `/tmp` de WSL | `scp.exe` es de Windows y no ve `/tmp` |
| `install` sin comprobar que el fichero llegó | falló limpio, pero por suerte |
| `rm -rf X && mv Y X` | el `scp` murió y la VM se quedó sin ninguna de las dos copias |

Las tres dan por hecho que el paso anterior funcionó. Es exactamente lo que
llevamos dos días arreglando en Rust, cometido en el shell tres veces en una
hora. La forma correcta lleva un `test -f` delante, y así se dictó a partir de la
tercera.

## 7. Lo que este punto deja sin resolver

**Los tres fragmentos de la clave de recuperación de `eje-prueba` están en el
mismo disco.** Con 2 de 3 juntos, esa clave es reconstruible por cualquiera con
acceso a esa máquina. Para un banco de pruebas es aceptable y se deja dicho; en
planta invalidaría el reparto entero. La propia herramienta lo avisa al generar,
y RPT-015 §8.1 ya advierte de la variante social del mismo problema.

**La asimetría con `revocaciones.rev` sigue en pie.** Su documentación dice que
su ausencia no es sospechosa *precisamente porque no hay testigo equivalente al
centinela*. Desde hoy la clave sí lo tiene, así que ese argumento hay que
rehacerlo o escribir por qué la asimetría sobrevive.

## 8. Puntos abiertos

| ID | Punto |
|---|---|
| PA-150 | **Cerrado.** §4 |
| PA-146b | El certificado de rotación, ya con un ancla en la que apoyarse |
| PA-149 | Dos listas de «manipulación» sin nada que las coteje. §5 |
| PA-145, PA-147, PA-148 | Sin cambios |

---

*Reporte Nº 96 — La memoria del centinela · PremosCorp · 10 de septiembre de 2026*
