# RPT-081 — Declarado no es cableado

**Tema:** PA-135. Cuatro de los seis canales estaban declarados y sin manejador, y nada lo decía
**Nº de reporte:** 081
**Fecha:** 26 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** Construido y probado. PA-135 parcial

- **Depende de:** RPT-006 §4 (los tres estados), RPT-079 §11.1 (la observación que lo destapó), RPT-004 §6.2 (la lista de permitidos)
- **Aborda:** PA-135. Abre PA-137, PA-138 y PA-139

---

## 1. La barrera llevaba el defecto escrito dentro

PA-135 se descubrió el 25 de agosto preguntándole a un agente de verdad, no leyendo
código. Cuatro de los seis canales rechazaban por falta de manejador.

Al buscar por qué ninguna prueba lo había cazado, apareció esto en la comprobación
que debía cubrir los seis:

```rust
// Los cuatro canales sin manejador se rechazan con motivo y eso ya tiene
// su prueba; aqui solo se miran los que responden de verdad.
for (canal, carga) in [
    (Canal::ConsultarAlertas, ...),
    (Canal::ObtenerCondiciones, ...),
] {
```

Una lista de dos escrita a mano **dentro** de la barrera, con un comentario que
justificaba dejar fuera a los otros cuatro. Es el séptimo índice a mano de la serie
y el único alojado en la propia comprobación que lo habría cazado: mientras esa
frase estuviera ahí, nadie iba a preguntarse por qué los otros no respondían.

**Y el hueco estaba probado como correcto.** Existe
`un_canal_sin_manejador_se_rechaza_con_motivo_y_no_con_lista_vacia`, es una buena
prueba, y hace que la ausencia se sienta resuelta. Es la forma más cómoda de no ver
algo: tener una prueba en verde que describe el síntoma.

## 2. Se rechazó podar, y por un motivo concreto

La propuesta del equipo técnico era retirar del contrato los dos canales sin
sustrato —`obtener-estado-boveda` y `consultar-sandbox`— con el argumento de que un
contrato debe reflejar capacidades presentes. El argumento es bueno.

**Pero podar no arregla el defecto.** Con los canales retirados, la lista de arriba
sería más corta y seguiría escrita a mano. Lo que hacía falta era que la barrera
**leyera del contrato** cuáles debe servir, y para leerlo el contrato tiene que
decirlo.

## 3. El contrato gana un tercer estado

```toml
servido = true     # el agente responde
servido = false    # declarado y sin manejador; se rechaza CON MOTIVO
# no declarado     # el guardian ni deja pasar la peticion
```

Es RPT-006 §4 aplicado al propio contrato: no «existe» contra «no existe», sino tres
estados. `servido = false` **no es una promesa de desarrollo**: es la declaración de
una ausencia, y cada una lleva escrito por qué.

La barrera nueva —`lo_que_el_contrato_declara_servido_responde_y_lo_demas_se_rechaza`—
recorre lo que el manifiesto declara. Un canal nuevo sin manejador rompe la suite el
mismo día en que se añade; un manejador nuevo sin actualizar `servido`, también.

Y del lado TypeScript, *«todo canal declara si está servido, y los que no dicen por
qué»*, por lo mismo que las demás: media barrera es como el punto de encuentro acabó
divergiendo (RPT-079 §2.1).

**Lo que no se hizo:** borrar la prueba de rechazo sin manejador. Se propuso, con el
argumento de que el 100% de la superficie tendría manejador. Esa prueba no protege
un canal: protege la doctrina de que «no hay nada» y «esto no lo sirve nadie» no se
confundan. Borrarla porque hoy no se alcanza es cómo se fabrica el próximo mecanismo
sin cablear.

## 4. `obtener-estado-agente`, cableado — y una guarda que faltaba

El encargo era atar `respuestaAutomatica` a `EstadoArranque::admite_contencion_automatica`.
**Es la mitad, y la mitad que falta es la peligrosa.**

Quien decide si el agente contiene por su cuenta son **dos** guardas independientes:

- `PerfilSegmento::permite_respuesta_automatica` — el perfil `ot` **nunca** la
  admite, y esa guarda ya decide en `evaluar`. IEC 62443 ordena las prioridades de
  una planta al revés que TI: una contención automática que detiene una línea **es**
  el incidente.
- `EstadoArranque::admite_contencion_automatica` — con el inventario suprimido no se
  contiene nada, diga lo que diga el perfil.

Con la instrucción tal cual, un sensor de planta con el almacén impecable habría
anunciado `respuestaAutomatica: true`: le habría dicho al operador que ese sensor
actúa solo cuando no lo hará jamás. De las dos formas de mentir, ésa es la mala.

Va como conjunción, con las cuatro combinaciones probadas: **una sola guarda cerrada
basta para cerrar el campo**.

### 4.1 Una tercera guarda que no llama nadie — PA-137

`boveda::VigenciaReglas::permite_respuesta_automatica` existe, está probada, y no
tiene un solo llamante. Mientras tanto la descripción del canal decía «según vigencia
de reglas»: el nombre prometía algo que el campo no daba.

No se incluyó inventando `Vigentes` — no hay distribución de reglas, y suponerla
sería un dato fabricado con aspecto de medida. **Se corrigió la descripción del
contrato** para que diga lo que el campo devuelve. Es lo contrario de PA-133, donde
la prosa iba por delante y nadie la siguió.

### 4.2 Dos `PerfilSegmento` no son deuda técnica

El cableado no compiló: `guardian_cc::PerfilSegmento` y
`eje_ipc::mensajes::PerfilSegmento` son tipos ajenos con el mismo nombre.

**No es un descuido.** `eje-ipc` depende de `thiserror` y `serde` y de nada más,
porque la capa de transporte no debe depender del núcleo de dominio; los dos se
comprueban contra el manifiesto por separado, y eso impide que diverjan. Un
`impl From` tendría que vivir en uno de esos dos crates —regla del huérfano— y
obligaría a invertir esa dependencia para ahorrar cuatro líneas.

La traducción es una decisión del agente, que conoce los dos lados, y vive ahí. El
`match` es exhaustivo: añadir un perfil **no compila**.

**Y el `match` exhaustivo no basta.** Obliga a traducir cada perfil; no impide
traducirlo mal. `Corporativo => Ot` compila igual de bien, y en el cable significaría
que la sala ve una oficina donde hay una planta — invitando a esperar una contención
automática que nunca va a ocurrir. Exhaustividad y corrección son dos preguntas, y
tienen dos pruebas.

## 5. `obtener-inventario`: se fue a escribir el productor

La discusión sobre este canal iba hacia un cambio de contrato: añadir una cuarta
postura y separar `contenido` a su propio campo, en un solo cambio atómico que cruza
contrato, `eje-ipc`, TypeScript, VIS-04 y la paridad de los dos extremos.

El razonamiento era correcto y el orden equivocado. **Nada ha construido jamás un
`NodoInventario`**: se construye en un solo sitio de todo el repositorio, una prueba
de reversibilidad en `eje-ipc`. Alrededor de esa forma que nadie produce hay cuatro
campos, dos enums, tipos en TypeScript, `resumirPostura` agregando en la vista, y
paridad en los dos lados. Todo verificado por **consistencia**; jamás ejercitado por
**verdad**.

Así que se fue a escribir el productor. Media hora.

## 6. Lo que salió — PA-138

**No se puede escribir**, y el motivo precede a todas las preguntas de diseño.

`AlmacenObservacion` guarda los dispositivos en dos colecciones privadas —`volatil`
y `pegajoso`— y **lo único público es contar cuántos hay**. No expone ninguna forma
de listarlos. Un canal que devuelve una lista de dispositivos descubiertos no puede
escribirse contra un almacén que no enumera.

No es un descuido del almacén: se diseñó para alimentar a `ProveedorHuella` y
`ProveedorSegmento`, que **preguntan por una MAC concreta**. Nunca fue un catálogo, y
forzar la exposición de sus colecciones sin decidir antes la política de ciclo de
vida habría roto el encapsulamiento por accidente.

Y hay **tres** colecciones candidatas a ser «el inventario vivo»: el volátil —sujeto
a expulsión—, el pegajoso —que no se olvida—, y el `vistos` que `main.rs` arma en
cada vuelta para la tabla por pantalla. Son cosas distintas y nadie ha tenido que
elegir cuál es.

### 6.1 De cuatro campos, hay fuente honesta para uno

| Campo | Fuente real hoy |
|---|---|
| `direccionEnlace` | ✅ la MAC observada |
| `identificador` | ❌ no existe origen. La MAC ya va en el otro campo |
| `clase` | ⚠️ exigiría traducir entre dos taxonomías distintas |
| `postura` | ❌ no hay valor para «no se sabe» (§7) |

El choque de taxonomías es el hallazgo más fino. `Indicio::sugiere()` devuelve
`ClaseExcluida` —`SoporteVital`, `SeguridadFuncional`—; el contrato pide
`ClaseDispositivo` —`plc|camara|medico|estacion|desconocido`—. **No son la misma
cosa:** una clasifica el equipo, la otra declara **la razón por la que no se puede
tocar**. «Soporte vital» no significa «médico»; significa que ninguna aprobación
levanta esa exclusión. Aplanar esa diferencia destruiría el criterio que impide
contener una bomba de infusión.

## 7. `Postura` mezcla un juicio con una medida — PA-139

`conforme | anomalo | contenido`. Los tres **afirman algo sobre el mundo**, y un
equipo visto en el cable sin marcado firmado no es ninguno de los tres.

Y `contenido` no es una postura: es un estado operativo. Contener a un equipo borra
la razón por la que se contuvo, que es justo lo que la sala necesita saber. Además
es **inalcanzable**: el agente no contiene nada (RPT-020).

El cambio —cuarta postura y contención a su propio campo— se hará **una vez y con
evidencia**, después de PA-138. Estabilizar hoy un contrato sobre una forma que nadie
ha producido es estabilizarlo sobre una suposición.

## 8. Lo que este día enseña sobre el método

Tres veces esta semana, intentar **usar** algo encontró en minutos lo que razonar no
encontró en días: la conversación de PA-78, el arranque del servicio de PA-133, y el
productor de hoy.

Y tres veces esta semana la propuesta recibida nombró elementos que no existen —
`ping` y `estadisticas`, `PerfilSegmento::Aislado`, `SinBase`/`Discrepancia`—. No es
descuido de nadie: es lo que le pasa a cualquiera que razona sobre un sistema sin
mirarlo. Es el mismo mecanismo por el que yo predije que ningún canal rechazaría
sabiendo que la paridad sólo comprueba declaración.

**La conclusión no es «probar más». Es no cerrar la forma de un mecanismo que nunca
se ha ejecutado.**

## 9. Puntos abiertos

| ID | Punto |
|---|---|
| PA-135 | 🔵 Parcial. `servido` en el contrato con barrera a los dos lados, y `obtener-estado-agente` cableado |
| PA-137 | `boveda::VigenciaReglas` existe, está probada y no la llama nadie |
| PA-138 | `obtener-inventario` no tiene productor posible: el almacén no enumera y tres de los cuatro campos no tienen fuente |
| PA-139 | `Postura` no tiene «no se sabe» y mezcla juicio con medida. Tras PA-138 |

---

*Reporte Nº 81 — Declarado no es cableado · PremosCorp · 26 de agosto de 2026*
