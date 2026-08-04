# Eje-Visión

Interfaz multiplataforma de Eje-Latam. TypeScript · React · Electron.

## Estado

Andamiaje pendiente. Este directorio queda reservado para no fijar decisiones de
herramienta antes de tiempo (empaquetador, gestor de estado, biblioteca de
componentes).

## Frontera de licencia

Este componente está **partido** por la frontera ratificada en RPT-003 §2.7:

| Módulo | Licencia |
|---|---|
| `VIS-01` Consola Eje-Almacén | Apache-2.0 |
| `VIS-03` Lanzador GUI | Apache-2.0 |
| `VIS-04` Panel de Confianza Cero e Inventario Vivo | Apache-2.0 |
| **`VIS-02` Tablero Directivo** | **Propietaria PremosCorp** |
| **`VIS-05` Mapa de Calor Regional** | **Propietaria PremosCorp** |

La partición debe reflejarse en la estructura de directorios desde el primer
commit de esta aplicación. Reorganizarla después es costoso y propenso a fugas de
código propietario al lado abierto.

## Restricción de transporte

La comunicación con `eje-agente` usa **IPC nativo del sistema operativo**: socket
de dominio Unix con ACL en Linux y macOS, named pipe con descriptor de seguridad
en Windows.

**No se expone un puerto TCP local.** Un WebSocket en `localhost` es accesible a
cualquier proceso local y a cualquier página web que el usuario visite — los
ataques de *DNS rebinding* contra servicios en `localhost` son un vector conocido
y explotado (RPT-002 §9.3).

## Dependencia declarada de VIS-05

El Mapa de Calor compara la postura del cliente contra el promedio sectorial de
Latinoamérica. Ese promedio **solo existe con agregación multiinquilino**, es
decir `NUC-01`, que es Fase 2. En Fase 1 `VIS-05` se limita a los datos del propio
despliegue, y así debe comunicarse en el material comercial (RPT-002 §9.6).
