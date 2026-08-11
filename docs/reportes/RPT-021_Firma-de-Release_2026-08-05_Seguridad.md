# RPT-021 — Firma de Release (Diseño)

**Tema:** Que el cliente pueda verificar de quién viene el binario
**Nº de reporte:** 021
**Fecha:** 5 de agosto de 2026
**Área designada:** Seguridad
**Entidad:** PremosCorp
**Estado:** **Diseño — sin implementar.** Requiere ratificación

- **Depende de:** RPT-011 §4 (`DominioClave`), RPT-004 §5 (firma del paquete empresarial), RPT-005 (firma híbrida)
- **Parte:** PA-14 en PA-14a, PA-14b y PA-14c
- **Cubre:** sólo PA-14a

---

## 1. PA-14 eran tres puntos con un número

Y mezclarlos habría producido un diseño que se contradice, porque **el verificador es distinto en cada uno** y eso decide todo lo demás.

| | Qué firma | Quién verifica | Estado |
|---|---|---|---|
| **PA-14a** | el binario que el cliente instala | el sistema operativo, antes de que exista código nuestro | 🔴 **bloquea el despliegue** |
| **PA-14b** | el paquete empresarial y los módulos en caliente | nuestro propio código, en ejecución | ✅ resuelto desde RPT-004 §5 |
| **PA-14c** | la atestación de conformidad PQC | una auditoría externa, fuera de línea | 🔵 post-MVP |

Este reporte cubre **sólo PA-14a**.

## 2. La decisión que no es nuestra

La pregunta natural era si la firma de release debía ser híbrida, como todo lo demás del proyecto. La respuesta es no, y no por criterio sino porque **no elegimos el verificador**.

Windows valida con Authenticode; ningún esquema poscuántico está aceptado por su cargador. Firmar con ML-DSA produciría un binario que el sistema no comprueba, y el usuario vería «editor desconocido» — habríamos **empeorado la seguridad real** para poder decir «poscuántico» en un folleto.

Es la aplicación del mismo criterio de RPT-008 §2 sobre los oráculos: quien escribió el comportamiento contra el que se prueba decide qué vale. Aquí quien escribió el verificador es Microsoft.

Donde sí controlamos las dos puntas —PA-14b— la firma híbrida sigue siendo la correcta y ya está en su sitio. **Las dos decisiones son coherentes precisamente porque son distintas.**

## 3. Lo que el CA/Browser Forum ya decidió por nosotros

Tres hechos que acotan el diseño antes de empezar, verificados y no recordados:

**La clave privada debe vivir en hardware.** Desde el 1 de junio de 2023, todo certificado de firma de código —EV **y** OV— exige que la clave se genere y quede no exportable en un módulo criptográfico certificado FIPS 140-2 nivel 2, Common Criteria EAL 4+ o equivalente. No es una recomendación de buenas prácticas: es condición de emisión.

Eso responde la pregunta 1 del equipo: **no hay elección entre HSM y fichero.** La única elección real es entre token físico y HSM en la nube del propio CA.

**FIPS 140-3 es el estándar activo.** Los requisitos citan 140-2 como mínimo, pero las certificaciones nuevas se emiten contra 140-3. Al comprar conviene apuntar a 140-3 y no tomar el mínimo por objetivo.

**La validez máxima baja a 460 días** para certificados emitidos desde el 1 de marzo de 2026. Un producto con vida de años renovará varias veces, así que la rotación no es un evento excepcional sino parte del ciclo.

## 4. El sellado de tiempo no es opcional

Sin sello, **la firma muere con el certificado**: cuando caduca, todo lo firmado antes deja de validar y las versiones ya instaladas empiezan a dar avisos de editor no confiable.

Con sello, la firma se evalúa contra el instante en que se firmó y no contra el actual, así que sobrevive a la caducidad y a la revocación posterior del certificado.

Con la validez recortada a 460 días, olvidar el sello significa que **cada release deja de validar en menos de año y medio**. Es el fallo silencioso más caro de este apartado: no se nota al publicar, se nota en la máquina de un cliente meses después.

## 5. Linux no tiene equivalente

No hay un Authenticode. Las opciones reales:

- **Paquete firmado en repositorio** —`.deb` o `.rpm` firmados con GPG—, que es lo que el gestor de paquetes verifica de forma nativa.
- **Firma separada junto a la descarga**, con la clave pública publicada. Funciona, pero la verificación pasa a ser un acto voluntario del administrador, y lo que no es automático no ocurre.

Para un producto que se instala en servidores de planta y hospital, **la primera es la única defendible**. Implica mantener un repositorio, que es infraestructura y no código.

## 6. Sobre el m-de-n: aquí la fricción juega en contra

PA-32 estableció reparto 2-de-3 para la clave de recuperación del cliente. **No se traslada.**

Aquella clave se usa una vez en años y la fricción era el objetivo. La de release se usa en cada versión, incluidas las que corrigen vulnerabilidades. Exigir custodios humanos en cada publicación convierte cada parche en una ceremonia, y el efecto probable no es más seguridad sino **releases más lentos** — que en un producto de seguridad es su propio riesgo.

La protección aquí la da el hardware no exportable del §3, no el número de personas.

## 7. Lo que este diseño no resuelve

1. **Es una compra, no un desarrollo.** Certificado, validación de identidad de la entidad y token o HSM. Semanas de plazo y coste recurrente. Ninguna línea de Rust lo acelera.
2. **La reproducibilidad de la construcción.** Firmar un binario dice quién lo publicó, no que corresponda al código del repositorio. Son garantías distintas y sólo cubrimos la primera.
3. **La renovación.** Con 460 días de validez, el procedimiento de rotación es parte del producto y no un trámite. Nadie lo ha escrito.
4. **`DominioClave::PremosCorp` no se usa para esto.** Ese tipo vive en `guardian-cc` y sirve para rechazar que la clave del proveedor firme inventarios del cliente. La firma de release ocurre **fuera del agente**, en el proceso de publicación, y no pasa por ese tipo. Conviene no confundir la frontera tipada con la clave física.

El punto 2 es el que más se parece a una promesa que nadie hizo pero que el cliente puede suponer.

## 8. Puntos abiertos

| ID | Punto | Bloquea |
|---|---|---|
| **PA-14a** | **Certificado de firma de código y su custodia en hardware.** Compra, validación de entidad, token o HSM, y sellado de tiempo obligatorio en el proceso de publicación | El despliegue en cliente Windows |
| **PA-46** | **Repositorio firmado para Linux.** Infraestructura de `.deb`/`.rpm` con GPG, o la decisión explícita de no cubrir Linux en Fase 1 | El despliegue en cliente Linux |
| **PA-47** | **Procedimiento de rotación.** Con 460 días de validez, renovar es parte del ciclo y no un trámite | Continuidad más allá del primer año |

---

*Reporte Nº 21 — Firma de Release (Diseño) · PremosCorp · 5 de agosto de 2026*
