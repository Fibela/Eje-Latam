/**
 * Matriz de capacidades según el estado de la licencia.
 *
 * Implementa RPT-003 §3.4 y RPT-004 §8.
 *
 * ## Principio
 *
 * **La licencia no controla si el módulo carga. Controla qué hace el módulo una
 * vez cargado.** La condición de carga es haber sido licenciado alguna vez, no
 * estarlo ahora.
 *
 * Condicionar la carga al estado de licencia dejaría a un comité de crisis
 * hospitalario sin tablero durante un incidente por una fecha de facturación.
 */

/** Estado de la suscripción para el nodo. */
export type EstadoLicencia =
  | "vigente"
  | "vencida-sin-incidente"
  | "vencida-con-incidente-activo"
  | "nunca-licenciado";

/** Capacidades de la interfaz habilitadas para un estado dado. */
export interface CapacidadesInterfaz {
  /** Si el paquete empresarial puede cargarse en absoluto. */
  readonly cargaModuloEmpresarial: boolean;
  /** VIS-02 muestra el estado operativo en vivo. */
  readonly tableroEnVivo: boolean;
  /** VIS-02 permite exportar reportes. */
  readonly exportacionReportes: boolean;
  /** VIS-05 permite comparativas históricas. */
  readonly comparativasHistoricas: boolean;
  /** CON-SIM permite ordenar simulacros. */
  readonly consolaSimulacion: boolean;
  /** El uso debe registrarse en ALM-01 para conciliación comercial. */
  readonly registrarUsoEnGracia: boolean;
}

/**
 * Resuelve las capacidades habilitadas para un estado de licencia.
 *
 * Nótese que ninguna combinación desactiva función de seguridad alguna: las
 * capacidades de `eje-agente` (AGT-01 a AGT-07) no aparecen en esta matriz
 * porque no son licenciables (RPT-003 §3.1).
 *
 * @param estado Estado actual de la suscripción.
 * @returns Capacidades habilitadas.
 */
export function capacidades(estado: EstadoLicencia): CapacidadesInterfaz {
  switch (estado) {
    case "vigente":
      return {
        cargaModuloEmpresarial: true,
        tableroEnVivo: true,
        exportacionReportes: true,
        comparativasHistoricas: true,
        consolaSimulacion: true,
        registrarUsoEnGracia: false,
      };

    case "vencida-sin-incidente":
      return {
        cargaModuloEmpresarial: true,
        tableroEnVivo: true,
        exportacionReportes: false,
        comparativasHistoricas: false,
        consolaSimulacion: false,
        registrarUsoEnGracia: true,
      };

    case "vencida-con-incidente-activo":
      return {
        cargaModuloEmpresarial: true,
        tableroEnVivo: true,
        exportacionReportes: true,
        comparativasHistoricas: true,
        // Un simulacro durante un incidente real es peligroso con independencia
        // de la licencia. Ver PA-15: esta restriccion es una propuesta de este
        // andamiaje y requiere ratificacion.
        consolaSimulacion: false,
        registrarUsoEnGracia: true,
      };

    case "nunca-licenciado":
      return {
        cargaModuloEmpresarial: false,
        tableroEnVivo: false,
        exportacionReportes: false,
        comparativasHistoricas: false,
        consolaSimulacion: false,
        registrarUsoEnGracia: false,
      };
  }
}

/**
 * Invariante verificable: ningún estado de licencia impide cargar el módulo si
 * alguna vez fue licenciado.
 *
 * @param estado Estado a comprobar.
 * @returns `true` si el estado corresponde a un nodo licenciado alguna vez.
 */
export function fueLicenciadoAlgunaVez(estado: EstadoLicencia): boolean {
  return estado !== "nunca-licenciado";
}
