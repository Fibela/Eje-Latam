# RPT-020 — Agente Mínimo y Extracción de Cabeceras

**Tema:** El primer programa que recorre el camino entero
**Nº de reporte:** 020
**Fecha:** 5 de agosto de 2026
**Área designada:** Red
**Entidad:** PremosCorp
**Estado:** Canónico con reservas explícitas — véase §6

- **Depende de:** RPT-018 (captura y observación), RPT-017 (arranque), RPT-009 (clasificación)
- **Cierra:** nada. Habilita PA-40
- **Abre:** PA-45

---

## 1. Por qué existía este reporte antes de PA-40

PA-40 decía «desplegar y ejecutar el agente». No era posible: `eje-agente/src/main.rs` imprimía cuatro líneas de configuración y terminaba. **No había nada que ejecutar.**

Es la cuarta vez que el proyecto acumula mecanismo verificado sin cablear —`disco.rs`, `ArchivoRevocaciones`, los tres centinelas— sólo que aquí lo ausente era el ejecutable entero.

## 2. Lo que el agente hace, y lo que no

Recorre por primera vez **captura → observación → clasificación → veredicto**. Hasta ahora cada pieza estaba verificada por separado y ninguna había visto un paquete.

Lo que **no** hace, escrito en su propio encabezado para que nadie lo suponga:

- **No carga inventario.** `arrancar` exige dos claves y no existe aprovisionamiento que las entregue. Opera como primer arranque, que es el estado honesto: sin marcados, la clasificación resuelve por segmento.
- **No contiene nada.** Calcula el veredicto y lo imprime. La emisión hacia un conmutador sigue bloqueada en PA-22.
- **No anexa a ALM-01.** Los manejadores de RPT-019 son PA-43.

Es un recorrido observable, no un servicio.

## 3. La etiqueta VLAN confirma RPT-018 §8.3

El diseño anticipaba que `ProveedorSegmento` y `ProveedorHuella` podrían dejar de ser independientes. Aquí está la razón concreta: **la etiqueta 802.1Q viaja en la misma trama que los puertos**. El segmento y la huella llegan por el mismo sitio, y separarlos en dos almacenes obligaría a mantener dos tablas con las mismas direcciones.

Y aparece un caso que el diseño no contemplaba: **en un puerto espejo sin etiquetar no hay VLAN**. Ahí el agente devuelve `NoDeclarado`, que RPT-009 §5 trata como si pudiera alojar críticos. Inventar «limpio» por comodidad convertiría un puerto sin etiquetar en permiso para contener.

### 3.1 Sólo doce bits son el identificador

Los cuatro altos de la etiqueta son prioridad y elegibilidad de descarte. Tomarlos por parte del identificador produciría segmentos inventados a partir de tráfico priorizado. `solo_los_doce_bits_bajos_son_el_identificador_de_vlan` lo fija.

## 4. La detección por puerto **no es huella**

Conviene decirlo con estas palabras, porque el nombre invita a creer más de lo que hay.

Un puerto no prueba un protocolo: Modbus movido al 10502 se escapa, y cualquiera puede abrir el 502 y hablar otra cosa. Esto es **extracción pasiva de cabeceras**, no inspección profunda ni análisis de firma de aplicación.

Se admite porque alimenta una fuente **inferida**, y por RPT-009 §3 esas sólo pueden sugerir criticidad, nunca descartarla:

| Error | Consecuencia |
|---|---|
| Falso negativo — Modbus en puerto raro | El dispositivo queda sin indicio, donde ya estaba |
| Falso positivo — alguien abre el 502 | Ambigüedad, y un humano decide |

**Ninguna de las dos direcciones concede permiso.** Esa es toda la razón por la que un mecanismo tan tosco es tolerable, y dejaría de serlo el día que alguien quisiera usarlo para declarar un equipo «no crítico».

La taxonomía de verdad es trabajo de dominio (RPT-018 §8.1); la tabla del agente deberá mudarse a donde viva esa decisión.

## 5. Verificación

`eje-captura` pasa de 8 a **15 pruebas**; el workspace, de 236 a **243**. Clippy y fmt limpios.

`una_trama_truncada_no_desborda_en_ningun_punto` recorta byte a byte una trama válida y llama al extractor con cada prefijo. Es donde vive el pánico a petición de quien emita la trama, y la razón de que todo acceso pase por `get`.

`una_trama_sin_transporte_no_es_un_fallo` fija una distinción que se pierde con facilidad: **no observar transporte y no poder leer la trama son cosas distintas**. La primera devuelve `Some` con `transporte: None`; la segunda, `None`.

### 5.1 En Windows el agente sale por donde debe

```text
Eje-Agente 0.1.0
Interfaz           : eth0
Error: Captura(PlataformaNoSoportada)
```

No finge una red silenciosa ni entra en el bucle. «No soportado» es admisible; simular una captura vacía habría sido peor que un error, porque una captura que nunca entrega tramas se parece a una red tranquila.

## 6. Reservas explícitas

1. **Nada de esto ha visto una red.** El extractor se probó contra tramas que escribí yo, con la forma que yo esperaba. Es el mismo sesgo que el arnés de RPT-014 corrige a medias: los casos los eligió quien escribió el código.
2. **`linux.rs` sigue sin ejecutarse.** Compila para el objetivo Linux; no ha abierto un socket.
3. **El agente no maneja señales.** Termina tras `--tramas N` o al agotarse el plazo sin tráfico. No es un demonio.
4. **La correspondencia VLAN → segmento es una simplificación.** Toda VLAN etiquetada se trata como `PuedeAlojarCriticos`. Lo correcto es consultar la declaración del administrador por identificador de VLAN, y esa declaración no existe: es parte de `ProveedorSegmento` real.

La reserva 4 es la que más se parece a una deuda de diseño y no a una limitación de alcance: hoy el agente no distingue una VLAN clínica de la de invitados.

## 7. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-45** | **Declaración de segmentos por VLAN.** El administrador declara qué identificador es clínico, de planta o limpio; hoy toda VLAN etiquetada se trata igual | Que la clasificación por segmento signifique algo |

---

*Reporte Nº 20 — Agente Mínimo y Extracción de Cabeceras · PremosCorp · 5 de agosto de 2026*
