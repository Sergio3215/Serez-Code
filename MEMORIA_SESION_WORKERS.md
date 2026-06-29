# Memoria de Sesión: Implementación y Optimización del Namespace `Task`

Este documento resume los aprendizajes de arquitectura, decisiones de diseño, restricciones sintácticas y optimizaciones aplicadas durante la implementación del namespace `Task` (concurrencia de subprocesos aislados) en el compilador de **Serez-Code** y su integración en la aplicación **Serez-Strike**.

---

## 1. Arquitectura de Concurrencia: `Task` (Share Nothing)

Se ha diseñado un modelo de concurrencia aislada basado en el principio de **no compartir memoria**:
* Cada worker se ejecuta en un hilo nativo de Rust (`std::thread`) y levanta su propia instancia aislada del Evaluador y de la región de memoria (GC independiente, sin compartir referencias de punteros).
* La comunicación se realiza únicamente por paso de mensajes serializados (`string`):
  * **Coordinador**: Inicia el worker con `Task.run("worker.sz", "input_string")`.
  * **Worker**: Lee el argumento con `Task.message()` y devuelve el resultado con `Task.reply("output_string")`.
  * **Coordinador**: Sondea el estado con `Task.isDone(id)` y recoge el resultado con `Task.poll(id)`.

---

## 2. Validación de Nombres Reservados en el Compilador

Para evitar ambigüedades en tiempo de ejecución y proteger las palabras clave del sistema, se ha blindado el compilador en [`src/parser.rs`](file:///E:/01%20Proyectos/Propio/Serez-code/src/parser.rs):
* **Regra**: Los nombres de los namespaces integrados del sistema (`Task`, `Time`, `DateTime`, `System`, `Gui`, `Dec`) son **nombres reservados**.
* **Efecto**: El parser prohíbe explícitamente declarar clases, interfaces o enums con estos nombres, arrojando un error de sintaxis estático durante la fase de análisis.
* **Test de error añadido**: [`tests/err_task_reserved_class.sz`](file:///E:/01%20Proyectos/Propio/Serez-code/tests/err_task_reserved_class.sz) valida que declarar una clase llamada `Task` falle en tiempo de compilación.

---

## 3. Restricción de JSX en Archivos de UI (`.szx`)

Durante la integración de la concurrencia en **Serez-Strike** ([`app.szx`](file:///E:/01%20Proyectos/Propio/serez-strike/app.szx)):
* **Conflicto**: Declarar diccionarios tipados explícitamente con corchetes angulares (ej. `let reqData <string, any> = (...)`) en archivos `.szx` provoca que el preprocesador de VDOM de Serez-UI intente interpretarlo como una etiqueta JSX/XML (`<string>`), corrompiendo la transpilación.
* **Solución**: Evitar el uso de `<...>` en la lógica inline de archivos `.szx`. Para pasar parámetros complejos entre hilos de forma segura, se debe preferir el empaquetado y serialización de datos en **arrays planos** `[...]` (como `let reqData = [method, url, headers, body]`), que no colisionan con JSX.

---

## 4. Optimizaciones de Rendimiento del Event Loop y GUI

Al integrar tareas en segundo plano en aplicaciones visuales nativas de **Serez-UI** (que utiliza renderizado por software en CPU mediante `softbuffer` y `cosmic-text`), se deben seguir estas dos directrices críticas de rendimiento:

### A. Sondeo de Tareas Espaciado (No polling continuo a 60 FPS)
* **Problema**: Forzar el refresco de pantalla a 60 FPS (`wantsFrames() = true`) continuamente mientras se espera la resolución de una petición de red consume ciclos de CPU del VDOM diffing de forma innecesaria, provocando micro-tirones.
* **Solución**: Limitar el sondeo de `Task.isDone(netTaskId)` en `onFrame()` a intervalos de **100ms** (10 FPS) mediante temporizadores. La UI permanece en reposo (0% CPU) el 90% del tiempo y se despierta únicamente en intervalos cortos para comprobar el estado, re-dibujando solo al completarse.

### B. Truncamiento de Visualización de Textos Gigantescos
* **Problema**: Intentar renderizar o scrollear un componente `Textarea` que contiene strings de gran tamaño (como un JSON de respuesta HTTP de 500 KB o 1 MB) colapsa el rasterizador de CPU, tirando los FPS a niveles mínimos durante el scroll.
* **Solución**: Truncar el texto visible en la interfaz a una longitud máxima manejable (ej. **10,000 caracteres** en `this.respView`) para el dibujo de la UI, y proveer un botón independiente para copiar el string real completo (`this.respPretty`) al portapapeles.
