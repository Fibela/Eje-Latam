/**
 * Lector de fuentes para las pruebas que inspeccionan código escrito a mano.
 *
 * No es un fichero de pruebas: no declara ninguna. Vive aquí porque sólo lo usan
 * las pruebas, y sacarlo a `src` lo colaría en el producto.
 *
 * # Por qué existe, y por qué está compartido
 *
 * Estaba dentro de `preload.prueba.ts`, y `vista.prueba.ts` lo necesitaba
 * también. Importar un fichero de pruebas desde otro habría vuelto a ejecutar
 * sus suites en el segundo proceso: los mismos casos contados dos veces, que es
 * la clase de cifra que hace creer que la cobertura subió.
 */

/**
 * Devuelve el fuente sin comentarios.
 *
 * ## Por qué hace falta un lexer y no basta un regex
 *
 * La primera versión de estas pruebas leía el fichero entero, comentarios
 * incluidos, y falló contra un comentario que citaba `window.eje = ...` como
 * ejemplo de lo que **no** hay que hacer.
 *
 * Ese fallo era el inofensivo. El peligroso es el simétrico: un comentario que
 * cite `ipcRenderer.invoke("obtener-inventario")` haría pasar la paridad **con
 * el método real borrado**. Falso negativo silencioso, en la única barrera que
 * protege el cuarto sitio donde vive el contrato.
 *
 * Es el mismo problema que `solo_codigo` resuelve en `xtask/src/cobertura.rs`
 * (PA-73), repetido en otro lenguaje. Se anota aquí para que la próxima prueba
 * que inspeccione fuentes empiece por esto.
 */
export function sinComentarios(fuente: string): string {
  let salida = "";
  let indice = 0;

  while (indice < fuente.length) {
    const dos = fuente.slice(indice, indice + 2);

    if (dos === "//") {
      const fin = fuente.indexOf("\n", indice);
      indice = fin === -1 ? fuente.length : fin;
      continue;
    }

    if (dos === "/*") {
      const fin = fuente.indexOf("*/", indice + 2);
      indice = fin === -1 ? fuente.length : fin + 2;
      continue;
    }

    const caracter = fuente[indice] ?? "";

    // Las cadenas se copian enteras: un `//` dentro de un literal no abre un
    // comentario, y tratarlo como tal se comería el resto de la línea.
    if (caracter === '"' || caracter === "'" || caracter === "`") {
      let cursor = indice + 1;
      while (cursor < fuente.length && fuente[cursor] !== caracter) {
        cursor += fuente[cursor] === "\\" ? 2 : 1;
      }
      salida += fuente.slice(indice, cursor + 1);
      indice = cursor + 1;
      continue;
    }

    salida += caracter;
    indice += 1;
  }

  return salida;
}
