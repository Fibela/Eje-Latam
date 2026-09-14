# RPT-100 — Estado a trece de septiembre

**Tema:** Reporte de situación pedido por la directiva. Qué dice el recuento, qué no dice, y qué falta de verdad
**Nº de reporte:** 100
**Fecha:** 13 de septiembre de 2026
**Área designada:** Gobernanza
**Entidad:** PremosCorp
**Estado:** Informativo. No cierra ningún punto

- **Depende de:** RPT-002 §12 (el tablero), RPT-021 §5 y §8 (los bloqueantes externos)

---

## 1. La cifra, y por qué sola no vale

```
Identificadores : 152
Cerrados        : 102     → 67 %
Parciales       :   7
Abiertos        :  43
Pendientes      :  50
```

**67 % de los identificadores están cerrados.** Es un dato real y sale del comando, no de
contarlo a mano.

Es también el número menos útil que se puede dar, por una razón que se ve mirando dos
fechas:

| | Identificadores | Cerrados | Pendientes | «Progreso» |
|---|---|---|---|---|
| 28-ago-2026 | 140 | 89 | 51 | 63,6 % |
| 13-sep-2026 | 152 | 102 | 50 | 67,1 % |
| **Diferencia** | **+12** | **+13** | **−1** | **+3,5 pp** |

En dieciséis días se cerraron trece puntos y se abrieron doce. **Los pendientes bajaron de
51 a 50.** El porcentaje sube porque el denominador crece con él.

Eso no es un fracaso: es lo que hace una auditoría que funciona. Cada punto nuevo salió de
arreglar otro —PA-149 salió de PA-150, PA-145 y PA-146 salieron de PA-144— y encontrarlos es
el trabajo, no un desvío del trabajo. Pero presentar el 67 % como «dos tercios del camino»
sería decir algo que el dato no sostiene.

## 2. La cifra que sí decide: **cinco**

Cinco puntos bloquean el despliegue. Ninguno se resuelve escribiendo Rust.

| ID | Qué falta | De qué depende |
|---|---|---|
| PA-14a | Certificado de firma de código, custodia en hardware, sellado de tiempo | Una compra |
| PA-46 | Repositorio firmado para Linux | Infraestructura y una decisión de distribución |
| PA-12 | Empaquetador verificable en las plataformas de destino | Depende de PA-14a y PA-46 |
| PA-39 | `CAP_NET_RAW` resuelto en la máquina del cliente | Un permiso en el entorno de destino |
| PA-61 | Segunda interfaz de red en la matriz de despliegue | Una condición de compra del hardware |

RPT-002 §12.1 ya lo dejó dicho el 6 de agosto y sigue siendo cierto cinco semanas después:
**el cuello de botella dejó de ser lo que falta por escribir y pasó a ser lo que falta por
comprar y por montar.**

Los otros cuarenta y cinco pendientes son deuda conocida. Desplegar con ellos abiertos es
una decisión de negocio, no un fallo de ingeniería.

## 3. Qué se puede afirmar del producto hoy

Lo que sigue está observado en máquina real, no deducido de que las pruebas pasen:

- El agente captura, clasifica, registra evidencia encadenada y sirve **cuatro de seis**
  canales por su socket local (RPT-090).
- VIS-04 pinta esos canales sobre un escritorio real, con dieciséis condiciones, y un canal
  que no puede servir **dice por qué** en lugar de fingir una lista vacía (RPT-093).
- La configuración firmada manda sobre la línea de órdenes, y el sensor se niega a arrancar
  si alguien intenta contradecirla (RPT-077).
- El almacén distingue supresión, sustitución y ceremonia pendiente para sus dos claves
  (RPT-096, RPT-099).

Lo que **no** se puede afirmar:

- Nadie ha instalado esto desde un paquete firmado en una máquina que no sea `eje-prueba`.
- La contención automática existe en código y **no se ha ejercido nunca contra tráfico real**.
- No hay medición de rendimiento en una red con carga.

## 4. El ritmo, medido

Trece puntos cerrados en dieciséis días, y de ellos **siete cerrados por observación en
máquina real** con captura o md5 comprobado en los dos lados. Ocho reportes escritos
(RPT-091 a RPT-099), cuatro con evidencia gráfica.

Y una cifra que importa más que las anteriores: **tres afirmaciones mías fueron falsas y se
retiraron dentro de las veinticuatro horas siguientes** —PA-151, la errata de RPT-094 §7 y
la predicción de la cabecera—. Las tres las cazó el repositorio: una regla vieja, una captura
de pantalla y una perturbación deliberada.

Un proyecto que no retira nada no está mirando.

## 5. Lo que este reporte arregla de paso

§12.1 llevaba dos listas escritas a mano, y las dos estaban desfasadas:

- La tabla de bloqueantes incluía **PA-40**, cerrado.
- La lista de deuda amarilla incluía **PA-53, PA-59 y PA-65**, cerrados.

Es el duodécimo índice a mano de la serie, en el mismo apartado que lleva escrito —desde
agosto— que las cifras a mano se retiraron porque envejecen en silencio. Se retiraron las
cifras y **se dejaron las listas**, que envejecen igual.

Se corrigen aquí. Lo que no se arregla hoy es la causa: mientras esas listas se escriban a
mano, volverán a desfasarse. Queda anotado como **PA-152**.

## 6. Qué sigue

| Plazo | Qué |
|---|---|
| Inmediato | PA-146b fase 3: cablear `EstadoOperativa` y observar una rotación real con dos custodios |
| Corto | PA-145, PA-147, PA-148 — tres puntos pequeños y frescos |
| Corto | PA-152: que el triaje de §12.1 se derive en vez de escribirse |
| **Bloqueante** | PA-14a y PA-46. **Nada de lo que escribamos los desbloquea** |

La recomendación operativa, dicha sin adornos: **el trabajo de ingeniería ya no es el camino
crítico.** Si el objetivo es un cliente en planta, el siguiente movimiento útil no es otro
punto del tablero sino la compra del certificado de firma y la decisión sobre el
repositorio.

---

*Reporte Nº 100 — Estado a trece de septiembre · PremosCorp · 13 de septiembre de 2026*
