# RPT-104 — El inventario que nadie podía aceptar

| | |
|---|---|
| **Tema** | El inventario que nadie podía aceptar |
| **Número** | 104 |
| **Fecha** | 2026-10-20 |
| **Área designada** | Agente |
| **Puntos** | PA-161 (cerrado), PA-162 (abierto), PA-163 (abierto) |
| **Precedentes** | RPT-099 (centinela v4), RPT-101 (PA-146b fase 3), RPT-103 (PA-158, PA-159) |

---

## 0. Una nota de reloj, antes de nada

Los sucesos de este reporte se midieron en el laboratorio de RUA con el reloj de
las máquinas virtuales, que corre en UTC. El equipo de Eje-Latam trabaja en
`America/Mexico_City`. La calma se corrió a las **03:23 UTC del 20 de octubre**,
que son las **21:23 del 19 de octubre** en la máquina donde se escribe esto.

Se fecha todo en **UTC**, porque es el reloj de la máquina que tomó la medida y
es el que aparece en el diario. Queda dicho aquí y no deducido después: PA-159 se
abrió porque un fichero de evidencia llevaba una fecha que no era la del día en
que se observó, y la lección de aquello no fue «poner bien las fechas» sino
**declarar contra qué reloj se fechan**. Dos equipos en dos zonas producen el
mismo suceso con dos fechas legítimas, y el que lo lea en marzo no tiene cómo
saberlo.

> **Pendiente menor que sale de aquí:** el proyecto no declara en ningún sitio
> cuál es su reloj de evidencia. Debería, en RPT-002, de una vez y para los dos
> equipos.

---

## 1. Qué se hizo: la primera ceremonia completa sobre una máquina nueva

Entre el 17 y el 20 de octubre de 2026 se aprovisionó `golden-eje-linux`, la
máquina de referencia del laboratorio de RUA, desde cero: construcción desde
árbol commiteado, ceremonia de firma completa en WSL con la semilla fuera del
alcance del laboratorio, entrega del paquete por `guestcontrol`, colocación de
los cinco artefactos y verificación de SHA-256 **en destino**.

Nunca se había hecho. `eje-prueba`, la máquina sobre la que se desarrolló toda la
maquinaria de identidad de las últimas seis semanas, llegó a su estado actual
**migrando** desde un centinela v3. Es la única puerta que existía, y por eso
nadie había comprobado que la otra —la del sensor nuevo— estuviera abierta.

No lo estaba. Y detrás de ella había un segundo muro que tampoco.

El laboratorio encontró **tres defectos del producto antes de correr uno solo de
sus propios escenarios**. Ése es el hecho central de este reporte, y conviene
decirlo sin adorno: ninguno de los tres lo podían haber encontrado las pruebas,
porque los tres viven exactamente en el hueco que las pruebas no ocupan.

---

## 2. PA-161 — la puerta que no existía

**Cerrado 18-oct-2026.**

Las anclas de identidad sólo las escribe `eje-manifiesto`. El agente no ancla
nunca, y eso es deliberado: anclar al primer uso convertiría el ataque en «borro
los dos ficheros y dejo la mía» (PA-150). El único comando que escribía anclas
era `migrar-centinela`, que **exige un centinela v2 o v3 existente**.

Un almacén recién generado no tiene ninguno. Y si se deja que lo cree el agente
al aceptar su primer inventario, sale **v4**, que `analizar_v2` también rechaza.

No había camino. Un sensor aprovisionado desde cero se quedaba con las dos anclas
en `None` para siempre, es decir con `operativaNoAprovisionada` y
`recuperacionNoAprovisionada` encendidas de forma permanente. Sustituirle la
clave no habría encendido nada — exactamente lo que PA-150 creía haber cerrado.

Entra **`eje-manifiesto anclar --almacen <dir>`**:

- Escribe el centinela v4 con las dos anclas y las marcas de agua **sin
  establecer**. No a cero: cero afirmaría haber aceptado la secuencia cero, y eso
  no es lo mismo que no haber aceptado nada (RPT-006 §4).
- **Se niega si ya hay centinela.** Reescribirlo pondría las marcas en «sin
  establecer» y a partir de ahí cualquier inventario o configuración antiguos y
  correctamente firmados volverían a aceptarse: el ataque de reversión servido
  por la herramienta que debería impedirlo.
- **Exige las dos claves públicas.** Anclar sólo una deja el sensor sin remedio
  si su identidad se compromete, y en un almacén nuevo las dos se acaban de
  crear: que falte alguna significa que la ceremonia se dejó un paso.

Comando aparte y no ampliar `migrar-centinela`: aquél **conserva** las marcas de
un sensor que ya vivió, éste las deja vacías porque no ha vivido nada. Juntarlos
daría una orden que a veces preserva y a veces no, decidiéndolo por lo que
encuentre en disco.

### 2.1 La prueba de la guarda, que hubo que reescribir

Cinco pruebas. La de la guarda —«se niega si ya hay centinela»— pasó en verde
estando mal, y la perturbación la delató.

Con el almacén de prueba **vacío**, retirar la guarda hacía caer la prueba. Verde
al perturbar, luego la prueba servía. Falso: caía por **la clave que falta**, no
por el centinela. Comprobaba el **texto** del error y la aserción que mira si el
fichero se tocó ni llegaba a ejecutarse. En un almacén real, con las dos claves
presentes, `anclar` habría reescrito el centinela en silencio.

Reescrita con las dos claves aprovisionadas, retirar la guarda hace que `anclar`
**tenga éxito**, y entonces la prueba cae porque no hubo negativa. Las dos
aserciones son distintas y hacen falta las dos: una caza «no se negó», la otra
«se negó pero escribió igual».

Es la segunda vez en el proyecto que una prueba comprueba el texto de un mensaje
creyendo comprobar un efecto. La regla que sale: **una prueba de guarda se
escribe sobre el estado en el que la guarda es la única razón posible de la
negativa**, y si para eso hay que aprovisionar media ceremonia, se aprovisiona.

---

## 3. La calma — lo que se vio llegar por primera vez

El 20 de octubre a las 03:23 UTC, desde el snapshot `base-identidad-anclada`
(`d520dd87`), con `systemctl start` y no `enable --now`, con el colector
escuchando en `192.168.56.1:5514` y `atacante-pa61` encendido en la red interna
aislada.

Tres latidos, a 30 segundos exactos:

```
msgid=latido-de-sensor  host=sensor-golden-01  interfaz=enp0s8
intervaloMs=30000  condiciones=[inventarioNoVerifica]  sello=0000…0000
pri=110  gravedad=6  facilidad=13  asiento=0
```

**Es la primera vez en la vida del proyecto que se ve algo llegar a un
colector.** Hasta ahora el camino de salida —latidos, sellos, alertas— estaba
construido, probado en bucle local y nunca observado de extremo a extremo sobre
dos máquinas y un cortafuegos de por medio.

Y `intervaloMs=30000` merece una frase propia. El código lleva 60000 por
omisión; 30000 está en `sensor.toml`, que viajó firmado y se aceptó. **PA-147
deja de ser una afirmación del diseño y pasa a ser una medida**: la
configuración firmada manda sobre el código, en máquina real, verificable en el
diario del colector.

---

## 4. PA-162 — el inventario que nunca se cableó

**Abierto. Es el hallazgo del laboratorio, y el más grave de los tres.**

El latido llegó con `condiciones=[inventarioNoVerifica]`. La predicción era
`condiciones=ninguna`, así que la predicción quedó **falsificada** — y lo que
había detrás no era un artefacto mal entregado.

### 4.1 El diagnóstico falso que se descartó primero

RUA llegó con una hipótesis razonada: la configuración verifica, luego la clave
operativa anclada es la correcta; si el inventario no verifica, estará firmado
por una clave anterior, de la corrida de ceremonia que PA-161 interrumpió.
Arreglo propuesto: re-emitir sólo `inventario.inv`.

Era falso, y re-emitir no habría cambiado nada. El inventario nuevo habría
chocado contra el mismo muro, y la corrida «habría confirmado» una teoría
equivocada. El salto que falla es el segundo: **la configuración y el inventario
no se entregan igual.**

- `anotar_configuracion`: *«El fichero de configuración no lo escribe el agente:
  lo deja el administrador en `/etc/eje-latam`. Aquí sólo se avanza la marca»*.
- `aceptar_inventario`: escribe **el centinela primero y el inventario después**,
  los dos, atómicamente, porque al revés quedaría un inventario cuya secuencia no
  consta y eso reabre la ventana de reversión.

La configuración se coloca a mano y por eso verifica. El inventario **no se
coloca a mano nunca**: lo escribe el agente al aceptarlo. Colocarlo produce la
única combinación que el diseño no contempla —fichero presente, marca sin
establecer— y entonces:

```rust
let Some(aceptada) = centinela.secuencia() else {
    return Err(ErrorInventario::FrescuraNoEstablecida);
};
```

`FrescuraNoEstablecida` es **incondicional y previa** a comparar firma o
secuencia. Con la marca en «sin establecer», ningún `inventario.inv` del mundo
verifica, lo firme quien lo firme.

RUA lo sacó de RPT-017, que lista `<datos>/inventario.inv — inventario firmado`.
Eso describe dónde el agente lo **guarda**, no dónde el administrador lo
**pone**, y ningún documento aclara la diferencia. **Es un defecto de nuestra
documentación, no un error de lectura suyo**, y RPT-017 hay que corregirlo.

### 4.2 El hallazgo de fondo

Buscando por dónde debía entregarse el inventario, apareció lo gordo:

**`aceptar_inventario` no tiene ningún llamador en producción.** Sólo las pruebas
de `guardian-cc`. Y `Canal::ObtenerInventario` no la usa: es de **lectura**
—`servicio.rs:223` sirve `self.inventario` al panel—, el panel preguntando qué
inventario tiene el agente, no un canal para entregarle uno nuevo.

Es la única función que avanza la marca de inventario. Luego:

- Con `inventario.inv` colocado a mano → `FrescuraNoEstablecida` para siempre.
- Sin colocarlo → `PrimerArranque` para siempre, porque el inventario no tiene
  por dónde llegar.

**Ningún sensor real puede alcanzar `EstadoArranque::Operativo` por ningún
camino.** Y como sin inventario aceptado no hay `MarcadoVerificado`, toda la
clasificación de criticidad resuelve por segmento: seis semanas de maquinaria de
reversión, revocación y frescura construidas sobre una puerta que nunca se
cableó.

### 4.3 Por qué estaba verde

Las pruebas llaman a `aceptar_inventario` **directamente**. La función está
probada, correcta y lo seguirá estando. Lo que no existe es el camino que la
llamaría — y una prueba que invoca la función no puede notar que nadie más la
invoca.

Es mecanismo-sin-cableado, la clase de defecto dominante de este proyecto, en su
forma más pura y en el peor sitio posible. Hermano de PA-161 y peor: aquél decía
que un sensor nuevo no podía anclar su identidad; éste dice que un sensor que la
ancló no puede aceptar un inventario jamás.

---

## 5. PA-163 — el motivo que no llega al registro

**Abierto. Lo encontró RUA midiendo lo que se le pidió.**

Se le pidió el `detalle` exacto del diario, para no actuar sobre una predicción.
Fue a buscarlo y **volvió diciendo que no está** — en vez de dar por bueno el
parecido entre la frase humana y la variante predicha. Esa negativa a cerrar el
círculo con una suposición es el trabajo correcto, y vale más que el verde que se
buscaba.

`EstadoArranque::NoVerifica` lleva un `detalle: String` con el `Display` de la
variante de `ErrorInventario` que falló. Hay **nueve**: firma de raíz inválida,
dominio de clave incorrecto, reversión detectada, clave revocada, frescura no
establecida, inclusión que no verifica, prueba ajena al marcado, dispositivo
duplicado, formato. Y el banner de arranque lo tira:

```rust
EstadoArranque::NoVerifica { .. } => "el inventario NO VERIFICA",
```

El `{ .. }` es el defecto. El campo no se imprime en ningún otro sitio.

**La asimetría lo delata:** `Configuracion::NoVerifica(motivo)` sí usa su motivo.
El del inventario se descarta. Y el comentario del propio campo dice
literalmente *«Motivo, para el registro forense»* — un campo cuyo propósito
documentado es el registro, que no llega a ningún registro.

Dos consecuencias, y la segunda la sufrimos esta semana:

1. El operador no puede distinguir una reversión de una firma inválida de un
   dominio equivocado. Tres incidentes con tres respuestas distintas, bajo una
   sola frase.
2. Durante el diagnóstico de PA-162 la variante concreta hubo que **deducirla
   leyendo el código**, porque la máquina que la había calculado no la decía. El
   laboratorio existe para que no haya que deducir.

Arreglo con forma conocida: al banner, y por IPC en `obtener-estado-agente`, con
la cautela de RPT-077 sobre no filtrar rutas del almacén en el texto del motivo.

---

## 6. El marcador de predicciones, incluida la mía que estaba mal formada

Se escribieron cinco predicciones **antes** de la corrida. El marcador honesto:

| | Predicción | Resultado |
|---|---|---|
| 1 | `intervaloMs=30000` | ✅ **Confirmada.** PA-147 real en máquina |
| 2 | `condiciones=ninguna` | ❌ **Falsificada.** `inventarioNoVerifica` → PA-162 |
| 3 | `dispositivos≥1` en el latido | ⚠️ **Mal formada.** Ver abajo |
| 4 | `sello-de-evidencia` presente | ✅ Confirmada, en `GENESIS` |
| 5 | Ningún `amenaza-incontenible` | ✅ Confirmada |

La 2 es la valiosa: una predicción falsificada que destapó el defecto más grave
del trimestre. Para eso se escriben antes.

### 6.1 La 3 estaba mal formada, y es la segunda vez

El latido no lleva un campo `dispositivos`. Su formato es:

```
latido={contador} interfaz={} sello={sello} asiento={numero}
intervaloMs={intervalo_ms} condiciones={}
```

Predije un campo que el formato de cable **no transporta**. No es una predicción
fallida: es una predicción que ninguna corrida podía responder, y por tanto no
medía nada. RUA la corroboró por el diario (`dispositivos=1`, DHCPDISCOVER del
atacante) y dejó anotado que no viaja en el `payload`, que es lo correcto.

**Es la segunda vez que cometo este error exacto.** La primera fue afirmar que
RUA debía esperar `condiciones=sinColector`, que es inalcanzable por
construcción porque `sinColector` está en `NO_EMISIBLES`. El patrón es el mismo:
razono sobre lo que el agente **sabe** en vez de sobre lo que el formato
**lleva**, y las dos veces habría hecho que un escenario rojo pasara por verde o
que un verde se buscara donde no puede estar.

La disciplina que adopto: **antes de predecir cualquier cosa sobre syslog, leer
la cadena de formato de ese mensaje.** No el contrato, no las condiciones, no la
estructura en Rust: la cadena literal que se escribe en el cable. Son treinta
segundos y las dos veces los habría ahorrado.

### 6.2 El sello en ceros no es un defecto

Sospeché que un `sello=0000…0000` emitido como si fuera un sello colapsaba «no
hay evidencia» en un valor concreto, violando la doctrina triestado. **Me
equivoqué, y RUA lo leyó bien.** El valor es `Resumen::GENESIS`, documentado como
*«Resumen de todo ceros, usado como antecesor del primer asiento»*: es el
antecesor legítimo de la cadena vacía, no una ausencia disfrazada. Con
`asiento=0` al lado, la lectura es inequívoca y coherente con PA-162 — sin
inventario aceptado no hay asientos que sellar.

Queda dicho porque una sospecha retirada por lectura del código vale lo mismo que
una confirmada, y porque el próximo que vea ceros en un sello no debería tener
que volver a dudarlo.

---

## 7. Lo que el laboratorio demostró sobre sí mismo

RUA se construyó para validar Eje-Latam. Antes de correr su primer escenario
propio ha entregado:

- **PA-161**, cerrado: la ceremonia de un sensor nuevo era imposible.
- **PA-162**, abierto: ningún sensor puede aceptar un inventario.
- **PA-163**, abierto: el motivo forense no llega al registro.
- **La primera observación de extremo a extremo** del camino de salida.
- **PA-147 medida** en lugar de afirmada.
- Dos trampas de operación documentadas —corrupción por apagado duro con
  `delalloc` de ext4, y el perfil `Public` del cortafuegos del anfitrión
  ignorando reglas `Allow`— que habrían costado días cada una la próxima vez.

Y una cosa más, que no es un hallazgo técnico: RUA llegó con un diagnóstico
equivocado, se le explicó por qué, **lo acató, no re-emitió nada, no tocó el
snapshot y reclasificó el muro como hallazgo del producto en su propio runbook**.
Un laboratorio que defiende su diagnóstico es un laboratorio que produce verdes
falsos. Éste no lo hizo.

El baseline `d520dd87` queda intacto. Es el punto de partida exacto para
demostrar el arreglo de PA-162: una máquina con identidad anclada y un inventario
imposible.

---

## 8. Qué queda

**Bloqueante, y es nuestro:**

- **PA-162** — cablear `aceptar_inventario`. Hay que decidir por dónde entra un
  inventario: un canal IPC de escritura con su autorización, o una orden de
  `eje-agente` que lo lea del disco y lo acepte. La decisión no es libre: lo que
  se elija tiene que dejar `FrescuraNoEstablecida` alcanzable sólo cuando alguien
  borró el centinela de verdad, que es lo que esa variante existe para decir.

**Deuda:**

- **PA-163** — el `detalle` al banner y a IPC.
- **RPT-017** — decir dónde se **coloca** cada artefacto y dónde el agente
  **guarda** los suyos. La ambigüedad costó un diagnóstico entero.
- **PA-160, PA-152** — siguen abiertos, sin cambio.
- Declarar el reloj de evidencia del proyecto (§0).
- Tarea #30 — `alertas.rs:770` sigue usando `matches!` sobre
  `EstadoArranque`. PA-153 se cerró para `EstadoOperativa` y
  `EstadoRecuperacion`; este sitio quedó fuera, y es justo el que derivó
  `inventarioNoVerifica` durante toda esta investigación. Añadir una variante a
  `EstadoArranque` hoy no pararía al compilador aquí.

**En RUA:**

- D1 — modelo mental 107/108 en los comentarios de `arnes/src/lib.rs`.
- D2 — escenarios estructurales rojos.
- D4 — corregir el reporte semanal del 19-oct (PA-152).
- Revertir el cortafuegos al cerrar el laboratorio:
  `Set-NetFirewallProfile -Name Public -AllowInboundRules False`.
