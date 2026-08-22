# RPT-057 — La sala se entera: el vigía, y por qué el par (asiento, sello) no prueba vida

**Tema:** PA-105. El detector de ausencia, el contador de latidos, y el cierre de PA-104 por observación
**Nº de reporte:** 057
**Fecha:** 13 de agosto de 2026
**Área designada:** Colector
**Entidad:** PremosCorp
**Estado:** **Implementado y verificado en ejecución real.** Cierra PA-105 y **PA-104**

- **Depende de:** RPT-052 (diseño del latido), RPT-053 (latido cableado), RPT-051 (opción D)
- **Aborda:** PA-104, PA-105. Abre PA-112, PA-113

---

## 1. RPT-052 §4 estaba equivocado, y había que corregirlo antes de construir

Aquel reporte dijo que un latido con el mismo par `(asiento, sello)` repetido es
sospechoso, porque el número de asiento es monótono y el extremo cambia con él.

**Eso es falso justo en el caso para el que existe el latido.** Un sensor en
calma no anexa nada: el registro no crece, el extremo no cambia, y dos latidos
legítimos separados por horas llevan exactamente el mismo par.

Con esa regla tal cual, **todo sensor tranquilo quedaría marcado como
sospechoso** — la fatiga de alertas que este proyecto lleva veinte reportes
evitando. Y al revés, que es lo grave: quien capture **un** latido y lo
reproduzca mantiene la sala en verde para siempre, porque el par sigue siendo el
correcto.

El error es del tipo que no se ve escribiendo el diseño: la propiedad invocada
—la monotonía del asiento— es cierta, y no dice lo que se le pidió que dijera.

## 2. El contador, y lo que no compra

La línea lleva ahora `latido=N`, monótono **en calma también**, porque cuenta
latidos y no asientos.

Se reserva antes de enviar y **sólo se consume si el envío funciona**. Así la
serie que llega al colector es contigua, y un hueco significa que alguien perdió
una línea por el camino. Quemar el número en un intento fallido produciría huecos
que no significan nada, y un hueco que a veces es normal deja de poder mirarse.

**Lo que no compra:** no detiene a quien reproduzca incrementando el contador.
La barrera que pone es que el atacante tenga que **seguir emitiendo**, no que le
baste con grabar un paquete. Es **PA-112**, y se dice aquí para que nadie lea el
contador como una firma.

> **Corrección de alcance (14-ago-2026).** Este párrafo decía que firmar el
> latido choca con RPT-038 §2 —«una clave local no sirve»— y eso **sólo vale
> para uno de los dos atacantes**:
>
> - **Quien compromete el sensor** tiene la clave y puede seguir firmando. Ahí sí
>   aplica RPT-038 §2 entero, y la firma no compra nada.
> - **Quien está en la red y no ha entrado en el sensor** —el que captura un
>   paquete y lo reproduce, que es el escenario que abrió PA-112— **no puede
>   firmar uno nuevo**. Una firma sobre `(contador, instante)` lo deja fuera.
>
> Son dos amenazas distintas y este reporte las trataba como una. PA-112 no es
> imposible: es **parcial**, y su valor exacto se puede escribir. Lo que sí lo
> bloquea es operativo: no hay aprovisionamiento de claves (PA-49, PA-51), y el
> agente arranca en `SinClaveAprovisionada` en todas las ejecuciones de estos dos
> días.

## 3. El vigía: tres decisiones de no decidir

`crates/eje-vigia`, sin dependencias, con la lógica separada del socket y
probada sin red. No es el SIEM del cliente: es la implementación más pequeña que
permite apagar un sensor y comprobar que alguien se entera, y una especificación
**ejecutable** para quien lo implemente en su herramienta. Una regla en prosa se
interpreta; ésta se ejecuta.

**`ReinicioORepeticion` es un solo estado.** El contador no sobrevive al proceso,
así que un reinicio legítimo retrocede — la misma forma que una repetición.
Elegir una sería inventarse la respuesta: un reinicio presentado como ataque
manda a alguien a responder a un incidente que no existe; una repetición
presentada como reinicio deja la sala en verde con el sensor silenciado. Se
declaran las dos y decide un humano.

**La hora es la del colector, no la de la línea.** La marca la escribe el sensor,
y en RPT-053 §3 ya se vio un reloj de pared retrocediendo. Además syslog no está
autenticado: quien pueda escribir en el canal puede fechar en el futuro, y eso
compraría silencio.

**Sin censo no se detecta al que nunca arrancó.** No hay ausencia donde no hubo
presencia: es «no se sabe», no «no hay» (RPT-006 §4). El censo tiene que salir de
la lista de despliegue del cliente y no de lo que el colector haya oído — uno
deducido de quien habla no puede echar de menos a quien nunca habló. El binario
lo avisa al arrancar si no se lo dan.

## 4. La observación que cierra PA-104

RPT-052 §6 puso la condición antes de escribir una línea de código: *«se cierra
cuando alguien apaga un sensor y la sala se entera»*.

Ejecución real, 13 de agosto de 2026:

```
Censo: LapTap-AF, sensor-fantasma
NUNCA VISTO  LapTap-AF: esta en el censo y no ha dicho nada.
NUNCA VISTO  sensor-fantasma: esta en el censo y no ha dicho nada.
LINEA BASE  LapTap-AF: primer latido (numero 1). Nada que afirmar todavia.
APARECE  LapTap-AF: informa por primera vez. Ya no falta del censo.
AUSENTE  LapTap-AF: sin latir desde hace 32212 ms (se le permitian 30000)
```

Con `--intervalo-latido 10000` y tolerancia de tres intervalos, la ventana son
30 000 ms y el aviso salió a los 32 212. La revisión corre cada cinco segundos,
así que la detección cae donde debe.

`sensor-fantasma` sigue en su sitio, callado tras el primer anuncio: es el
control de que el mecanismo **distingue**, no de que calla.

**PA-104 cerrado por observación.** Y conviene anotar que hicieron falta cuatro
intentos, y que ninguno de los tres primeros falló por el mecanismo: puerto
ocupado, identidad equivocada (RPT-058) y orden de arranque invertido.

## 5. Lo que la prueba de fuego ejercitó de paso

En el intento con el orden invertido —el agente arrancó sin colector escuchando—
se vio por primera vez en campo el camino de fallo completo: el latido no salió,
`Latido::NoSePudo` encendió `salidaNoDisponible`, y el agente lo dijo con la
frase que importa: *«para la sala, este sensor es indistinguible de uno
muerto»*. Hasta ese momento esa rama sólo existía en pruebas.

## 6. Puntos abiertos

| ID | Punto |
|---|---|
| ~~PA-104~~ | ✅ **Cerrado por observación**, no por construcción (§4) |
| ~~PA-105~~ | ✅ Detector de referencia, con lógica probada sin red |
| **PA-112** | El contador no resiste a quien reproduzca incrementándolo. Exige firmar el latido, y RPT-038 §2 dice por qué la clave no puede ser local (§2) |
| ~~PA-113~~ | ✅ Identidad compuesta, cerrada por observación en RPT-059 §5 |
| PA-41 | La tolerancia de tres intervalos es hipótesis declarada, no medida |
| PA-79 | `--intervalo-latido` es provisional: quien controle el arranque alarga la ventana que la sala vigila |

---

*Reporte Nº 57 — La sala se entera · PremosCorp · 13 de agosto de 2026*
