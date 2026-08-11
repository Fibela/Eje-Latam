# RPT-041 — La respuesta de alertas deja de ser un array desnudo

**Tema:** Por qué `consultar-alertas` pasa a devolver `{ primerDisponible, sucesos }`
**Nº de reporte:** 041
**Fecha:** 8 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-74

- **Depende de:** RPT-019 (manejadores), RPT-035 (protocolo), RPT-040 (segmentación)
- **Cierra:** PA-74

---

## 1. El hueco lo abrió PA-59, no un descuido

Antes de la segmentación, un array desnudo era una respuesta honesta: el agente
tenía todo el registro y devolvía lo que le pedían. Tras PA-59 el agente carga
**sólo el segmento activo**, y esa misma respuesta pasó a ser una vista parcial
con apariencia de exhaustividad.

Un cliente que pide `desdeAsiento: 0` recibe las alertas del segmento activo y no
tiene forma de saber que hubo diez mil asientos antes. La lista es correcta. Lo
que falta no es validez, es **saber dónde empieza**.

Es la tercera vez esta semana que aparece la misma forma: «no hay nada» y «esto no
está aquí» son estados distintos, y colapsarlos es cómo un operador concluye que
un incidente no ocurrió (RPT-006 §4).

## 2. Por qué no es un error

La alternativa evidente era devolver `AsientoFueraDeRango` cuando
`desdeAsiento < base`. Se descartó, y conviene que quede escrito por qué:

**La petición es legítima.** Quien pide desde cero quiere todo lo que haya; no se
ha equivocado en nada.

**La respuesta es correcta.** Las alertas devueltas existen y son exactas.

Con un error, el cliente no recibe **ni siquiera las alertas vivas**, y la reacción
natural de quien integra es capturar el fallo y reintentar desde un número
inventado. Habríamos cambiado una vista parcial silenciosa por una vista vacía
ruidosa más un cliente adivinando desplazamientos.

## 3. `primerDisponible` significa lo que sobrevive en disco

No «lo que este canal alcanza». Si significara eso, la cifra cambiaría el día que
exista un canal para leer segmentos archivados, sin que el registro haya cambiado
en absoluto — y un auditor leería una pérdida de evidencia donde sólo hubo un
cambio de interfaz.

## 4. Se lee del disco en cada consulta, y no se cachea

Ésta es la decisión que más discusión costó y la que más importa.

Cachear la cifra al arrancar es barato y **la convierte en mentira exactamente en
el caso que la hace valiosa**: si alguien borra `evidencia-000001.alm` con el
agente en marcha, una cifra cacheada sigue diciendo `1` y el agente afirma que hay
evidencia disponible desde el asiento 1 cuando ese tramo ya no existe.

Es un dato que se queda obsoleto **en la dirección que oculta la manipulación**.
De las dos direcciones posibles, es la peor.

Lo que se ahorraba: un `read_dir` sobre unas decenas de entradas, muy por debajo
del coste de serializar las hasta 256 alertas de la propia respuesta. Se pagaba
una falsedad posible por una fracción del coste de la respuesta que la contiene.

La prueba `si_alguien_borra_el_segmento_archivado_la_cifra_lo_refleja` es ese
argumento hecho ejecutable: rota, comprueba que la cifra es 1, borra el archivado
y exige que suba. Con caché, esa prueba falla.

## 5. La base se lee de la cabecera, no del nombre del fichero

`evidencia-000002.alm` dice **qué segmento es**, no **en qué asiento empieza**. Hoy
son deducibles el uno del otro porque el umbral es constante; dejarían de serlo el
día que alguien lo cambie o que un segmento se cierre antes de tiempo.

Cuesta un `read` por segmento y elimina una clase entera de desajuste.

## 6. Si el directorio no se puede leer

Se devuelve la base del segmento activo: lo único que consta con certeza.
Devolver `1` afirmaría que hay histórico sin haberlo comprobado, que es inventarse
una garantía justo en el momento en que el disco está fallando.

## 7. Lo que faltaba, encontrado antes de dar el punto por cerrado

El tipo `RespuestaAlertas` estaba declarado en `puente.ts`, la prueba de paridad
pasaba, y **la firma del contrato seguía diciendo
`Promise<readonly SucesoAlerta[]>`**. El tipo existía y no lo usaba nadie.

La paridad comprueba que los campos declarados coinciden entre el manifiesto y el
código; no comprueba que el contrato del puente use el registro que acaba de
declararse. Es un hueco real de la barrera de PA-20, y esta vez se vio a mano.

Queda anotado como límite conocido: **la paridad valida esquemas, no usos.**

## 8. Los siete sitios

`contrato-ipc.toml`, `mensajes.rs` (registro + `CAMPOS_RESPUESTA_ALERTAS`),
`alertas.rs` (`primer_disponible`), `servicio.rs` (el manejador y la ruta que
ahora necesita), `main.rs`, `puente.ts` (tipo, campos **y firma**) y las dos
pruebas de paridad.

`consultar` sigue devolviendo un `Vec` porque la emisión hacia el testigo lo
consume así: el envoltorio se construye en la frontera del IPC, que es donde el
contexto hace falta. La ruta de emisión no se tocó.

## 9. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| ~~PA-74~~ | — | ✅ **Cerrado por este reporte** |
| **PA-75** | **La paridad valida esquemas, no usos.** Un registro declarado y no usado por el contrato pasa las dos pruebas | Que un tipo nuevo quede sin cablear sin que nadie lo note |

---

*Reporte Nº 41 — La respuesta de alertas deja de ser un array desnudo · PremosCorp · 8 de agosto de 2026*
