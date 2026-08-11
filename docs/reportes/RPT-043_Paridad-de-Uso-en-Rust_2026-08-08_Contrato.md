# RPT-043 — La paridad de uso, en el otro extremo del cable

**Tema:** Que el manejador de Rust sirva la forma que el manifiesto declara, comprobado por lo que sale y no por cómo está escrito
**Nº de reporte:** 043
**Fecha:** 8 de agosto de 2026
**Área designada:** Contrato
**Entidad:** PremosCorp
**Estado:** **Implementado.** Cierra PA-76

- **Depende de:** RPT-041 (respuesta de alertas), RPT-042 (paridad de uso en TypeScript)
- **Cierra:** PA-76

---

## 1. Conductual, no textual

En TypeScript la barrera de RPT-042 lee el fuente de `puente.ts`, y no había
alternativa: allí el contrato **es** la declaración, no hay implementación que
ejecutar.

En Rust sí la hay. Así que esta barrera no mira cómo está escrito el `match` de
`servicio.rs`: **llama al manejador y compara las claves del JSON que produce**
con los campos que el manifiesto declara para ese canal.

La diferencia no es de estilo. Un análisis textual se rompe cuando alguien
reformatea, extrae un método o renombra una variable — y se rompe *hacia el lado
malo*, dejando de comprobar sin dejar de pasar. Esto observa lo que llega al
cable, que es lo único que le importa a quien está al otro lado.

## 2. Sin tabla que mantener

El canal sale del manifiesto, la forma de respuesta de ese canal sale del
manifiesto, y los campos de esa forma salen del manifiesto.

Se relee el TOML en `eje-agente` en lugar de reutilizar los ayudantes de
`eje-ipc` —que son `#[cfg(test)]` de otro crate— porque la alternativa era un mapa
`canal → CAMPOS_*` escrito a mano en el fichero de pruebas. Veinticinco líneas de
lectura duplicada frente a una tabla que hay que cuidar: la duplicación es la
opción que no se desincroniza.

## 3. Lo que no encontró

**Nada.** El manejador ya servía exactamente lo que el manifiesto declara.

Conviene decirlo con el mismo énfasis con que se han dicho los ocho hallazgos
anteriores. Es la primera vez en nueve intentos que una barrera nueva encuentra el
terreno limpio, y un reporte que sólo destaca cuando caza algo acabaría midiendo
el entusiasmo del que escribe en lugar del estado del sistema.

Lo que sí falló fueron dos importaciones mías de más —`desenmarcar` aquí, los
grupos de captura en RPT-042—, que es otra categoría: ruido de escritura, no
divergencia de contrato.

## 4. Lo que la barrera no cubre

**Sólo mira las respuestas.** Los bloques `direccion = "peticion"` no se
comprueban aquí. Están parcialmente cubiertos por otra vía: las peticiones se
deserializan con `deny_unknown_fields`, así que un campo de más falla en
ejecución. Un campo **de menos** o renombrado, no.

**Los cuatro canales sin manejador quedan fuera.** Se rechazan con motivo y eso ya
tiene su prueba desde RPT-036 §6; aquí sólo se miran los dos que responden de
verdad. El día que alguno de los cuatro se implemente, entrará en el bucle sin que
nadie toque esta prueba — es la ventaja de derivarlo del manifiesto.

**No comprueba el orden de las claves.** La paridad de `eje-ipc` ya lo verifica
contra el manifiesto y serde serializa en orden de declaración. Repetirlo aquí
sería una garantía duplicada que aparenta cobertura adicional sin añadirla.

## 5. El circuito, cerrado

| Frontera | Barrera | Reporte |
|---|---|---|
| Campos declarados ↔ código, en ambos lenguajes | Paridad de esquemas (PA-20) | RPT-011 y siguientes |
| Manifiesto ↔ firma del puente TypeScript | Paridad de uso, textual | RPT-042 |
| Manifiesto ↔ lo que el manejador de Rust sirve | Paridad de uso, conductual | Este reporte |

## 6. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| ~~PA-76~~ | — | ✅ **Cerrado por este reporte** |

---

*Reporte Nº 43 — La paridad de uso, en el otro extremo del cable · PremosCorp · 8 de agosto de 2026*
