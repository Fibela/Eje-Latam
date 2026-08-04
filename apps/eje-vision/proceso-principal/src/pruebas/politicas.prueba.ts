/**
 * Pruebas de las invariantes de política.
 *
 * Cada prueba corresponde a una restricción que un reporte canónico declara como
 * no negociable. Si alguna falla, el fallo no es de implementación: es una
 * violación de política que debe elevarse antes de tocar el código.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  capacidades,
  fueLicenciadoAlgunaVez,
  type EstadoLicencia,
} from "../estado-licencia.js";
import {
  CANALES_PERMITIDOS,
  CANALES_PROHIBIDOS,
  esCanalPermitido,
  validarPeticion,
} from "../puente-ipc.js";
import {
  destinoExternoPermitido,
  PREFERENCIAS_SEGURIDAD,
} from "../seguridad-ventana.js";

const ESTADOS: readonly EstadoLicencia[] = [
  "vigente",
  "vencida-sin-incidente",
  "vencida-con-incidente-activo",
  "nunca-licenciado",
];

describe("RPT-003 §3.4 — la licencia no controla si el modulo carga", () => {
  it("todo nodo licenciado alguna vez puede cargar el modulo", () => {
    for (const estado of ESTADOS) {
      assert.equal(
        capacidades(estado).cargaModuloEmpresarial,
        fueLicenciadoAlgunaVez(estado),
        `estado ${estado}: la carga debe depender de haber sido licenciado, no del estado actual`,
      );
    }
  });

  it("durante un incidente activo VIS-02 opera sin restriccion", () => {
    const durante = capacidades("vencida-con-incidente-activo");
    assert.ok(durante.tableroEnVivo);
    assert.ok(durante.exportacionReportes);
    assert.ok(durante.comparativasHistoricas);
  });

  it("una licencia vencida sin incidente conserva el tablero en vivo", () => {
    const vencida = capacidades("vencida-sin-incidente");
    assert.ok(vencida.tableroEnVivo, "el tablero en vivo nunca se retira");
    assert.ok(!vencida.exportacionReportes, "la exportacion si se retira");
  });

  it("el uso en gracia queda registrado para conciliacion", () => {
    assert.ok(capacidades("vencida-sin-incidente").registrarUsoEnGracia);
    assert.ok(capacidades("vencida-con-incidente-activo").registrarUsoEnGracia);
    assert.ok(!capacidades("vigente").registrarUsoEnGracia);
  });
});

describe("RPT-004 §6.2 — el puente IPC es una lista de permitidos", () => {
  it("ningun canal prohibido figura entre los permitidos", () => {
    for (const prohibido of CANALES_PROHIBIDOS) {
      assert.ok(
        !esCanalPermitido(prohibido),
        `el canal '${prohibido}' no puede existir en el puente`,
      );
    }
  });

  it("no existe pasamanos generico", () => {
    assert.ok(!esCanalPermitido("invocar"));
    assert.ok(!esCanalPermitido("ejecutar-comando"));
  });

  it("la contencion no es alcanzable desde la interfaz", () => {
    assert.ok(!esCanalPermitido("ordenar-contencion"));
  });

  it("un canal desconocido se rechaza", () => {
    assert.deepEqual(validarPeticion("canal-inventado", 10), {
      admitida: false,
      motivo: "canal-desconocido",
    });
  });

  it("una carga excesiva se rechaza aunque el canal sea valido", () => {
    assert.deepEqual(validarPeticion("obtener-inventario", 99_999_999), {
      admitida: false,
      motivo: "carga-excesiva",
    });
  });

  it("los canales permitidos se admiten", () => {
    for (const canal of CANALES_PERMITIDOS) {
      assert.deepEqual(validarPeticion(canal, 128), { admitida: true, canal });
    }
  });
});

describe("RPT-004 §6.1 — configuracion de seguridad de la ventana", () => {
  it("el aislamiento de contexto esta activo y la integracion de Node no", () => {
    assert.equal(PREFERENCIAS_SEGURIDAD.contextIsolation, true);
    assert.equal(PREFERENCIAS_SEGURIDAD.nodeIntegration, false);
    assert.equal(PREFERENCIAS_SEGURIDAD.sandbox, true);
    assert.equal(PREFERENCIAS_SEGURIDAD.webSecurity, true);
  });

  it("las preferencias estan congeladas", () => {
    assert.ok(Object.isFrozen(PREFERENCIAS_SEGURIDAD));
  });

  it("solo se abren destinos externos explicitamente permitidos", () => {
    assert.ok(destinoExternoPermitido("https://premoscorp.com/soporte"));
    assert.ok(!destinoExternoPermitido("https://premoscorp.com.atacante.io/"));
    assert.ok(!destinoExternoPermitido("http://premoscorp.com/"));
    assert.ok(!destinoExternoPermitido("file:///etc/passwd"));
    assert.ok(!destinoExternoPermitido("no es una url"));
  });
});
