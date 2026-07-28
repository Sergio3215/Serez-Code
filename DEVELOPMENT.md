# Serez-Code — Development Reference

**Para quien desarrolla el lenguaje en Rust**: arquitectura del intérprete,
decisiones técnicas, suite de tests, tooling, CI/CD y estado del proyecto.

Si lo que querés es *programar en* Serez-Code, el documento es el
[README](README.md): instalación, referencia del lenguaje y semántica. Este
archivo asume que vas a tocar `src/`.

---

## Índice

1. [Estado del proyecto](#1-estado-del-proyecto)
2. [Estructura del repositorio](#2-estructura-del-repositorio)
3. [Arquitectura del intérprete](#3-arquitectura-del-intérprete)
4. [Pipeline de ejecución](#4-pipeline-de-ejecución)
5. [Modelo de memoria — regiones y arenas](#5-modelo-de-memoria--regiones-y-arenas)
6. [Evaluador — submódulos](#6-evaluador--submódulos)
7. [Suite de tests](#7-suite-de-tests)
8. [Apps demo](#8-apps-demo)
9. [Extensión VS Code](#9-extensión-vs-code)
10. [CI/CD — Release pipeline](#10-cicd--release-pipeline)
11. [Seguridad del repositorio](#11-seguridad-del-repositorio)
12. [Cómo construir y testear](#12-cómo-construir-y-testear)
13. [Convenciones para contribuir al core](#13-convenciones-para-contribuir-al-core)
14. [Limitaciones conocidas del lenguaje](#14-limitaciones-conocidas-del-lenguaje)
15. [Pendiente](#15-pendiente)
16. [Apéndice — features implementadas (histórico)](#16-apéndice--features-implementadas-histórico)

---

## 1. Estado del proyecto

| Métrica | Valor |
|---|---|
| Versión | 9.11.0 (`Cargo.toml`) |
| Archivos Rust | 55 (`src/`) |
| Tamaño del parser | ~136 KB |
| Tamaño del evaluador (total submódulos) | ~1.2 MB |
| Archivos de test `.sz` | 402 (`tests/`) |
| Extensión VS Code | v1.9.0 |
| Binarios | `sz` (CLI) y `sz-lsp` (Language Server) |
| Plataformas de release | Windows (MSI + .exe), Linux x64 (estático), macOS ARM64 e Intel |

> El conteo de tests que pasan no se anota acá a propósito: queda desactualizado
> en cuanto se agrega un test. Corré `.\run_tests.ps1` / `./run_tests.sh` para el
> número real del momento.

---

## 2. Estructura del repositorio

```
serez-code/
├── src/                        — Código fuente Rust
│   ├── main.rs                 — CLI: file run, REPL, --check, --watch, --version, install
│   ├── token.rs                — Enum Token + lookup_ident() para keywords
│   ├── lexer.rs                — Scanner byte-indexed sobre la String fuente
│   ├── ast.rs                  — Nodos del AST: Statement, Expression, BlockStatement
│   ├── parser.rs               — Parser Pratt (TDOP), 8 niveles de precedencia
│   ├── type_checker.rs         — Checker estático pre-ejecución
│   ├── region.rs               — Arena allocator, ObjectRef, ObjectData, OwnedValue
│   ├── scope.rs                — ScopeStack: push/pop/lookup con watermarks
│   ├── repl.rs                 — Read-eval-print loop
│   ├── package_manager.rs      — serez.json, install_package, install_all, packages_dir
│   ├── test_run.rs             — Helper interno para tests
│   ├── lsp_main.rs             — Entry point del binario sz-lsp (stdio JSON-RPC)
│   ├── lsp/                    — Language Server (6 módulos)
│   │   ├── server.rs               — Loop LSP: initialize, didOpen/didChange, publishDiagnostics
│   │   ├── analysis.rs             — Símbolos del documento, hover, go-to-definition
│   │   ├── builtins.rs             — Catálogo de namespaces/métodos para completado
│   │   ├── builtins_gen.rs         — GENERADO por tools/gen_lsp_builtins.py (no editar a mano)
│   │   ├── rpc.rs                  — Framing JSON-RPC sobre stdio
│   │   └── mod.rs
│   ├── compiler/               — Backend nativo (work in progress)
│   │   ├── types.rs                — Tipos de compile-time (SzType) → tipos LLVM
│   │   ├── hir.rs / hir_lower.rs   — HIR: AST desugarizado + pase de lowering
│   │   ├── mir.rs / mir_lower.rs   — MIR: three-address code con basic blocks
│   │   ├── llvm_emit.rs            — MIR → texto LLVM IR
│   │   └── mod.rs
│   └── evaluator/              — Intérprete tree-walking (28 submódulos)
│       ├── mod.rs
│       ├── stmt.rs
│       ├── expr.rs
│       ├── ops.rs
│       ├── check.rs
│       ├── builtins.rs             — assert, type_of, parseInt, fetch, time, env, exit
│       ├── classes.rs
│       ├── methods_array.rs
│       ├── methods_string.rs
│       ├── methods_set.rs
│       ├── methods_tensor.rs       — relu, sigmoid, tanh, softmax, abs, pow, sqrt, exp, log, norm, clamp, broadcastAdd
│       ├── namespaces.rs           — Math, File, JSON
│       ├── namespaces_crypto.rs    — sha256, md5, hmacSha256, base64, hex
│       ├── namespaces_socket.rs    — Socket.connect/send/recv/close/listen/accept
│       ├── namespaces_binary.rs    — Binary.fromHex/toHex/fromUtf8/packInt32Le…
│       ├── namespaces_gpu.rs       — GPU.createBuffer/map/reduce/dot/matmul…
│       ├── namespaces_memory.rs    — Memory.sizeof/alloc/free/read/write/copy/fill/offsetOf
│       └── control.rs
│
├── tests/                      — Suite de tests (.sz)
│   ├── framework.sz            — Framework de unit testing
│   ├── packages/               — Paquetes locales para tests (SEREZ_PACKAGES)
│   │   ├── math-helpers/index.sz
│   │   ├── string-tools/index.sz
│   │   └── serez.json
│   ├── unit_*.sz               — 105 tests unitarios
│   ├── sec_*.sz + err_*.sz     — 75 tests de seguridad y error
│   ├── demo_*.sz               — 3 demos
│   └── NN_*.sz + *.expected    — 70 tests E2E con golden files
│
├── apps/                       — 5 apps demo (ejercitan todo el lenguaje)
│
├── tools/                      — Utilidades de desarrollo (Python)
│   ├── gen_lsp_builtins.py     — Regenera src/lsp/builtins_gen.rs desde el evaluador
│   └── lsp_smoke.py            — Smoke test: maneja una sesión LSP real por stdio
│
├── vscode-serez/               — Extensión VS Code
│   ├── extension.js            — DocumentFormattingEditProvider
│   ├── package.json            — Manifest v0.2.0
│   ├── language-configuration.json
│   └── syntaxes/serez.tmLanguage.json
│
├── wix/main.wxs                — Configuración instalador MSI (usado por cargo-dist)
├── dist-workspace.toml         — Configuración cargo-dist para releases
├── .github/
│   ├── workflows/release.yml   — CI/CD pipeline de release
│   └── dependabot.yml          — Actualizaciones automáticas semanales
│
├── run_tests.ps1               — Test runner (Windows/PowerShell)
├── run_tests.sh                — Test runner (Linux/macOS/Bash)
├── Cargo.toml                  — Metadata del proyecto Rust
├── README.md                   — Doc para QUIEN PROGRAMA EN serez-code (referencia del lenguaje)
├── CHANGELOG.md                — Historial técnico de cambios por versión
├── DEVELOPMENT.md              — Este archivo: doc para QUIEN DESARROLLA EL LENGUAJE
└── bugs.md                     — Log de bugs documentados (todos corregidos)
```

---

## 3. Arquitectura del intérprete

El intérprete sigue el pipeline clásico de 4 etapas sin dependencias externas de runtime:

```
Fuente .sz
    │
    ▼
Lexer (lexer.rs)
    — Scan byte-indexed sobre &str (sin Vec<char>)
    — 1-char lookahead para tokens de 2 chars (==, !=, <=, >=, =>)
    — Emite Token { type, literal, line, column }
    │
    ▼
Parser (parser.rs ~100KB)
    — Pratt TDOP: parse_program() → Program { Vec<Statement> }
    — Prefix handlers: literals, identifiers, if, fn, class, enum, arrays, dicts, ( )
    — Infix handlers: operadores aritméticos, comparación, lógicos, bitwise, power,
      f(args), a[i], obj.method(args), obj?.method(args), a ?? b, a is T
    — Error recovery: synchronize() salta a ; o } o keyword en caso de error
    │
    ▼
TypeChecker (type_checker.rs)
    — Pase estático sobre el AST antes de ejecutar
    — Recolecta todas las FunctionDeclarations en un mapa nombre → firma
    — Infiere tipos para variables let con RHS literal o resultado de call
    — Verifica call sites contra parámetros y tipos de retorno declarados
    — Reporta a stderr; NO detiene la ejecución
    │
    ▼
Evaluator (evaluator/ — 28 módulos)
    — Tree-walking interpreter
    — Flash Scope protocol en cada bloque { }
    — Scratch watermark para temporales de out en top-level
    │
    ├── stdout  (out statements, REPL)
    └── stderr  (errores de parser, type checker, runtime)
```

### Decisiones de diseño clave

| Decisión | Alternativa descartada | Razón |
|---|---|---|
| Arena allocator + watermarks | GC / Rc<RefCell<T>> | Determinístico, O(k) por scope, zero unsafe |
| `ObjectRef { region, index }` | Raw pointers / Box | No puede dangling: index inválido ≠ memoria inválida |
| `Rc<BlockStatement>` para fn bodies | Clone del AST | Clonar una función es O(1) en lugar de O(n) |
| `StoredClass` con 4 HashMaps | Vec<ClassMethod> lineal | Dispatch O(1): methods, static_methods, getters, setters |
| Pratt TDOP parser | Recursive descent clásico | Precedencia de operadores fácil de extender |
| Zero `unsafe` | — | Invariante de seguridad no negociable |

### Lexer — scan byte-indexed

El lexer trabaja directamente sobre la `String` fuente con offsets de bytes
(`position`, `read_position`); NO copia la entrada a un `Vec<char>`. Los
caracteres UTF-8 multibyte en identificadores funcionan igual porque `read_char`
avanza `c.len_utf8()` bytes y el sliceo de strings usa `&str[start..end]`, que es
indexado por rango de bytes.

### Parser — Pratt TDOP

Top-Down Operator Precedence, 8 niveles. Todo operador infijo debe registrarse en
**dos lugares** de `parser.rs` (ver §14 para el procedimiento completo); hacerlo
en uno solo produce comportamiento sutilmente incorrecto: el parser ignora el
operador o descarta silenciosamente la expresión que lo rodea.

---

## 4. Pipeline de ejecución

### Modos del CLI

| Comando | Comportamiento |
|---|---|
| `sz archivo.sz` | Ejecuta el archivo completo |
| `sz` | REPL interactivo |
| `sz --check archivo.sz` | Profiler estático (estimación de bytes por función) |
| `sz --watch archivo.sz` | Re-ejecuta automáticamente al guardar |
| `sz --version` | Imprime la versión |
| `sz install` | Instala dependencias de `serez.json` desde el registry |
| `sz install pkg@version` | Instala un paquete específico desde el registry |

### Flujo de `sz archivo.sz`

```
1. Leer fuente del disco
2. Lexer → Vec<Token> (implícito en el parser)
3. Parser → Program { statements }   [errores a stderr]
4. TypeChecker → pase sobre el AST    [errores a stderr]
5. Evaluator → ejecuta statements     [errores a stderr, out a stdout]
```

### REPL

El REPL reutiliza el mismo pipeline por línea. Mantiene un `Evaluator` persistente entre líneas para que las variables declaradas en una línea sean visibles en las siguientes.

---

## 5. Modelo de memoria — regiones y arenas

Dos cosas distintas que conviene NO mezclar (el README las mezclaba):

- **El modelo de memoria** es *region-based memory* con arena allocators. Los
  valores viven en arenas y salir de un ámbito libera la región de ese ámbito de
  una sola vez. No hay GC. Eso es arquitectura: el programador no lo maneja.
- **El Flash Scope** es la **feature del lenguaje** montada sobre ese modelo: el
  bloque `{ ... }` INTERNO que el programador escribe dentro de una función o
  método —no las llaves del cuerpo de la función, un bloque adentro del cuerpo—
  para acotar a mano la vida de sus temporales. Es la herramienta con la que él
  decide dónde empieza y termina una región. Su caso de uso es el volumen de
  RAM: construir la estructura grande dentro de las llaves, quedarse solo con la
  parte que va a usar —en una variable declarada ANTES del bloque— y soltar todo
  lo demás en la llave de cierre. El `return` va DESPUÉS del bloque:

  ```serez
  fn int sumar(int a, int b) {
      let res = 0;
      { res = a + b; }   // ← el flash scope
      return res;
  }
  ```

  Las mismas llaves funcionan también en el top level, fuera de toda función.

Esta sección documenta el modelo. El uso del constructo, con ejemplos, está en el
[README](README.md#flash-scopes).

### Dos arenas

Ambas son un `Vec<ObjectData>` plano (`region.rs`).

```
Global Arena
  — Sembrada al arrancar: singletons null/true/false + cache de ints 0..=256
  — Variables top-level, funciones, clases y CELDAS de closure
  — Persiste toda la vida del programa: solo se achica con el scratch
    watermark de `out` (único caso en todo el runtime)

Scoped Arena
  — Variables locales, argumentos, temporales de bloque
  — Stack de watermarks: una entrada por scope activo
  — Cleanup: arena.truncate(watermark) — sin GC
```

`alloc()` elige arena por PROFUNDIDAD, no por valor: si hay al menos un frame
activo alloca en la Scoped; si no, en la Global. El mismo literal cae en una u
otra según dónde aparezca.

### ObjectRef

Cada valor en el intérprete es una referencia segura:

```rust
ObjectRef { region: RegionId, index: usize }
```

- `region`: Global o Scoped — determina qué arena leer
- `index`: posición dentro del Vec de la arena
- Nunca es *memory-unsafe*: es un índice a un `Vec` seguro, el peor caso es
  fuera de rango → `None` (`Referencia inválida`), nunca memoria liberada
- **Pero el índice NO queda inaccesible al truncar**: los slots se reutilizan y
  un ref viejo resolvería a OTRO objeto vivo. La garantía real la da el
  protocolo de abajo (ninguna ref sobrevive a su scope), no el índice

### Protocolo de scope (invariante "promote before pop")

`push`/`pop` están apareados en TODOS los code paths, incluidos errores y
salidas tempranas. El extract/plant solo ocurre si algo escapa del bloque:

```
1. scopes.push()               — graba watermark
2. evaluar statements del bloque
3. extract(result_ref)         — deep clone a OwnedValue (arena-independent)
4. scopes.pop()                — trunca arena: libera todos los locales
5. plant(owned)                — re-alloca en el scope padre
```

El caso canónico que justifica el invariante — el valor que escapa es un array
cuyos elementos viven en el frame que se está por liberar:

```serez
fn make_pair(int a, int b) {
    return [a, b];          // el array vive en el frame scoped de la función
}

let p = make_pair(10, 20);  // extract antes del pop, plant en la arena global
out p[0];                   // → 10 — seguro, ahora vive en la global
out p[1];                   // → 20
```

Los pasos 3 y 5 se SALTEAN cuando no escapa nada:

| Caso | Qué se promueve |
|---|---|
| `if`/`else`, `switch`, `try`/`catch`/`finally`, bloque suelto, `unsafe` (`eval_block`) — y brazos de `match` (push/extract/pop/plant inline) | el valor del bloque, `return` y `throw` |
| Cuerpo de función/método | solo el `return` (o el payload del `throw`) |
| Cuerpo de loop — `while`, `do-while`, `for`, `for-in` (`eval_block_discard`) | solo `return`/`throw`; **el valor del bloque se descarta a propósito** |
| `break`, `continue`, labels, `Error` | nada: no hay payload |

El descarte en cuerpos de loop no es un detalle: sin él, un cuerpo cuyo último
statement produce un compuesto (`arr = arr.map(...)`) plantaba una COPIA COMPLETA
por iteración en el frame de arriba, liberada recién al salir del loop —
400 MB medidos en 300 iteraciones sobre un array de 20k.

### Por qué los valores se copian

`extract` es un **deep clone** del árbol completo (arrays anidados, entradas de
dict, campos de instancia). De ahí sale la semántica por valor del lenguaje:
pasar un argumento, retornar un compuesto o leer una variable copia los datos,
así que dos scopes nunca aliasan el mismo slot.

El costo: los compuestos van EMBEBIDOS — un array ocupa UN slot con un
`Vec<OwnedValue>` adentro, no un slot por elemento. Extract/plant es
O(tamaño total) ⇒ retornar un array de 100k copia 100k elementos y `a[i] = x`
sobre un array grande es O(n). Para números pesados, `Tensor` (un `Vec<f64>`
plano en un solo slot).

### Costo real del cleanup

`arena.truncate(watermark)` dropea los slots por encima de la marca: es
proporcional a los slots liberados (`len - watermark`) MÁS el costo de dropear
lo que cada slot contiene — un slot con un array de 1M es un drop de 1M, no un
destructor suelto. Lo que gana el modelo no es un drop más barato por objeto,
sino de dónde sale la cota: está acotado por los datos del propio scope y se
paga en un punto exacto del código, en vez de recorrer todo el heap vivo cuando
lo decide un GC.

### Excepción: variables capturadas por closures

Un closure puede correr mucho después de que muera el bloque que declaró sus
variables. Por eso, al crear una función o lambda, cada local capturada se
**promueve**: `extract` de la scoped → `plant_global` → `rebind_ref` reapunta
TODOS los bindings que miraban al slot viejo (la variable original y `this` en
cada frame de una cadena de métodos anidados; reapuntar solo el frame más
interno bifurcaba el objeto). Consecuencias:

1. **La variable capturada sobrevive al bloque** — vive en la arena global, que
   el protocolo de scope nunca trunca.
2. **Deja de copiarse**: closure y scope exterior comparten UNA celda, así que
   una mutación adentro se ve afuera y viceversa. Es lo contrario de la
   semántica por valor, es deliberado (semántica de celda) y es el único lugar
   del lenguaje donde dos nombres comparten storage.
3. **Cuesta memoria permanente**: los slots globales no se reclaman nunca. Por
   eso `capture_lambda_env` captura solo los nombres que el cuerpo menciona de
   verdad — capturar todas las locales visibles filtraba un slot permanente por
   local no usada por creación de lambda (letal en lambdas por frame o por
   iteración).

```serez
fn counter() {
    let n = 0;              // promovida a celda global cuando la lambda la captura
    return () => { n = n + 1; return n; };
}
let next = counter();
out next();   // → 1
out next();   // → 2 — la celda sobrevivió al return
```

### Optimizaciones de arena

| Colección | Capacidad inicial |
|---|---|
| Ambas arenas (`Arena::new()`) | 64 objetos |
| Frame de scope | 4 entradas |

`Arena::new()` es la misma para la global y la scoped: las dos arrancan en 64.
La global además se siembra con ~260 objetos (null/true/false + ints 0..=256),
o sea que crece más allá de su capacidad inicial antes del primer statement; a
cambio, los enteros chicos y los booleanos se entregan como refs existentes en
vez de alocarse en cada uso. `global_bindings` y los registries de
interfaces/clases/enums son `HashMap::new()` y crecen bajo demanda.

---

## 6. Evaluador — submódulos

El evaluador original era un solo archivo de 5300+ líneas. Hoy son 28 módulos cohesivos; los principales:

| Módulo | Responsabilidad principal |
|---|---|
| `mod.rs` | Entrada, Flash Scope protocol, StoredClass (4 HashMaps O(1)), profiler |
| `expr.rs` | Todas las expresiones: calls, index, dot, ternary, interpolation, namespaces |
| `stmt.rs` | Todos los statements: let, assign, for, while, if, class, enum, import… |
| `classes.rs` | Instanciación, dispatch, herencia, super, getters/setters |
| `methods_array.rs` | 20+ métodos de array |
| `ops.rs` | Infix (aritmética, bitwise, power, comparación) y prefix |
| `namespaces.rs` | Math, File, JSON namespaces |
| `namespaces_crypto.rs` | Crypto: sha256, md5, hmacSha256, base64, hex |
| `namespaces_socket.rs` | Socket: connect, send, recv, close, listen, accept |
| `namespaces_binary.rs` | Binary: fromHex, toHex, fromUtf8, packInt32Le/Be, matmul… |
| `namespaces_gpu.rs` | GPU: createBuffer, map, reduce, dot, axpy, matmul (CPU-backed) |
| `builtins.rs` | parseInt, parseDecimal, readLine, y otros globals |
| `methods_string.rs` | 20+ métodos de string |
| `methods_set.rs` | add, has, delete, clear, toArray, union, intersection |
| `methods_tensor.rs` | Operaciones tensoriales (Tensor namespace) |
| `check.rs` | Type-check de parámetros, return, typed arrays |
| `control.rs` | Break, continue, labeled loops, do-while |

### Helpers estructurales (reducen duplicación)

| Helper | Reemplaza |
|---|---|
| `print_call_stack()` | Loop de 3 líneas para imprimir la cadena de calls — en cada sitio de error |

Vive en `evaluator/mod.rs`, junto al protocolo de scope.

### Internals de rendimiento

Optimizaciones que evitan clones y allocs redundantes en los caminos calientes.

**`Rc<BlockStatement>` — clonar una función es O(1).** Todo valor función guarda
su cuerpo AST como `Rc<BlockStatement>` en vez de un `BlockStatement` propio.
Leer una función de la arena, pasarla como callback o devolverla desde
`find_method` incrementa un refcount en lugar de deep-clonar el cuerpo. Aplica
tanto a `OwnedValue::Function` como a `ObjectData::Function` (`region.rs`).

**`StoredClass` — dispatch de métodos O(1).** Los métodos de clase se guardan en
`StoredClass` con cuatro `HashMap` separados: `methods`, `static_methods`,
`getters` y `setters`; cada lookup es O(1) por nombre. Los `StoredMethod` llevan
`body: Rc<BlockStatement>`, así que cada clone es O(1) sin importar el tamaño del
método. Antes, cada llamada clonaba el `ast::ClassMethod` completo con su cuerpo.

**Dedup en `all_bindings()`.** `ScopeStack::all_bindings()` recorre los frames de
adentro hacia afuera y saltea los nombres ya vistos. Cuando un closure captura su
entorno, las variables externas sombreadas no se extraen ni se re-alocan: cada
nombre aparece a lo sumo una vez en el entorno capturado.

---

## 7. Suite de tests

### Estructura

| Categoría | Cantidad | Descripción |
|---|---|---|
| `unit_*.sz` (no sec) | 83 | Tests unitarios usando `framework.sz` (assert, expect) |
| `NN_*.sz` + `.expected` | 57 | Tests E2E con golden files — diff exacto de stdout |
| `err_*.sz` | 27 | Verifican que ciertos inputs producen error de runtime |
| `sec_*.sz` | 41 | Suite de seguridad: overflow, OOB, null safety, stack overflow |
| `unit_sec_*.sz` | 15 | Tests unitarios de seguridad (con framework.sz) |
| CLI / REPL / --check | 13 | Tests de modo de ejecución del CLI |
| `framework.sz` | 1 | Framework compartido por todos los unit tests |
| **Total** | **274** | **0 fallando** |

### Test runners

**Windows (PowerShell):**
```powershell
.\run_tests.ps1                    # suite completa
.\run_tests.ps1 -unit              # solo unit tests
.\run_tests.ps1 -e2e               # solo E2E + error tests
.\run_tests.ps1 -security          # solo security tests
.\run_tests.ps1 -filter "switch"   # filtrar por nombre
.\run_tests.ps1 -generate          # regenerar .expected
```

**Linux / macOS (Bash):**
```bash
./run_tests.sh                     # suite completa
./run_tests.sh --unit
./run_tests.sh --e2e
./run_tests.sh --security
./run_tests.sh --filter "switch"
./run_tests.sh --generate
```

### Convenciones de naming

- `unit_<feature>.sz` — test unitario de una feature específica
- `unit_sec_<tema>.sz` — test unitario de seguridad
- `sec_<escenario>.sz` — test de error: debe fallar con runtime error
- `err_<escenario>.sz` — test de error: debe fallar con error
- `NN_<nombre>.sz` + `NN_<nombre>.expected` — test E2E numerado
- `tests/_*.sz` — ignorados por git y por los runners (archivos de debugging temporal)

---

## 8. Apps demo

Cinco programas en `apps/` que ejercitan todas las features del lenguaje en conjunto. Cada uno es autocontenido y ejecutable con `sz apps/<nombre>.sz`.

| App | Features principales |
|---|---|
| `01_task_manager.sz` | `enum`, herencia (`UrgentTask : Task`), `static` methods, `switch`, HOF (filter/map/reduce), `try/catch/throw` |
| `02_statistics.sz` | Typed arrays `[decimal]`, `Math` namespace, map/filter/reduce para estadísticas, histograma, correlación de Pearson |
| `03_text_analyzer.sz` | String methods (split, replace, trim, indexOf, charAt, padEnd), dicts para frecuencia de palabras, cifrado César, `File` I/O |
| `04_bank_system.sz` | `abstract class`, `sealed class`, `interface`, `const`, getters (`get`), `try/catch/throw`, `?.`, `??` |
| `05_data_pipeline.sz` | `JSON` (stringify/parse), `File` (write/read), `Set` (deduplicación), bitwise (`&`, `\|`, `^`), power (`**`, `>>`), pipeline HOF |

---

## 9. Extensión VS Code

### Versión 0.2.0 (`vscode-serez/`)

| Archivo | Rol |
|---|---|
| `extension.js` | Activación + `DocumentFormattingEditProvider` |
| `package.json` | Manifest: lenguaje serez, gramática, formatter, configDefaults |
| `language-configuration.json` | Brackets, autoclose, indentationRules |
| `syntaxes/serez.tmLanguage.json` | Gramática TextMate para syntax highlighting |

### Formatter (`extension.js`)

El formatter implementa `DocumentFormattingEditProvider` con las siguientes reglas:

- **Indentación**: 4 espacios por nivel, basada en conteo de `{` / `}`
- **Strings y comentarios**: el conteo de llaves ignora contenido dentro de `"..."` y después de `//`
- **`} else {`**: dedent antes de imprimir la línea, indent después — manejado correctamente
- **Líneas en blanco**: máximo una consecutiva
- **Trailing whitespace**: eliminado en todas las líneas
- **EOF**: el archivo siempre termina con exactamente un `\n`

### Configuración automática para `.sz`

```json
"[serez]": {
    "editor.defaultFormatter": "sergio.serez-code",
    "editor.formatOnSave": true,
    "editor.tabSize": 4,
    "editor.insertSpaces": true
}
```

### Rebuild del .vsix

```powershell
cd vscode-serez
vsce package          # genera serez-code-0.2.0.vsix
antigravity-ide.cmd --install-extension serez-code-0.2.0.vsix
```

El `.vsix` está en `.gitignore` — es un artefacto de build, no código fuente.

---

## 10. CI/CD — Release pipeline

### `release.yml` — GitHub Actions

El workflow se activa al hacer push de un tag con formato semver (`1.0.0`, `v0.1.0`, etc.).

**Jobs:**

| Job | Permisos | Función |
|---|---|---|
| `plan` | `contents: read` | Corre `dist plan` para determinar qué builds hacer |
| `build-local-artifacts` | `contents: read` | Compila binarios para cada plataforma + crea instaladores nativos |
| `build-global-artifacts` | `contents: read` | Genera checksums y artefactos globales |
| `host` | `contents: write` | Sube artefactos y crea el GitHub Release |
| `announce` | `contents: read` | Notificaciones post-release |

**Plataformas de release:**

| Plataforma | Artefacto |
|---|---|
| `x86_64-pc-windows-msvc` | `sz.exe` + instalador `.msi` (via WiX) |
| `x86_64-unknown-linux-gnu` | `sz` + shell installer |
| `aarch64-unknown-linux-gnu` | `sz` (ARM64 Linux) |
| `x86_64-apple-darwin` | `sz` (macOS Intel) |
| `aarch64-apple-darwin` | `sz` (macOS Apple Silicon) |

**Herramienta:** `cargo-dist v0.28.0` — gestiona todo el proceso de empaquetado y release.

### Seguridad del CI

- Permisos **mínimos por job**: solo `host` tiene `contents: write`
- El resto de jobs tienen `contents: read` explícito
- `dependabot.yml` actualiza actions y dependencias Cargo cada lunes

### `.github/dependabot.yml`

```yaml
# github-actions: pineará @v4 → SHA fijo automáticamente
# cargo: actualiza Cargo.toml cuando hay nuevas versiones
schedule: weekly (lunes)
```

---

## 11. Seguridad del repositorio

### `.gitignore`

| Patrón ignorado | Razón |
|---|---|
| `*.sz` | Archivos de desarrollo/prueba local |
| `!tests/*.sz` | Excepción: tests son fuente de verdad |
| `tests/_*.sz` | Archivos probe/debug temporales |
| `*.txt`, `*.json`, `*.bin` | Outputs de runtime (análisis, pipeline, binarios) |
| `*.vsix` | Artefacto de build de la extensión |
| `/target` | Directorio de build de Cargo |
| `/.claude/` | Configuración local del editor |

### Archivos de documentación ignorados (histórico)

`Serez-Code-Internals.md`, `AUDIT.md`, `implementacion_clases.md` — documentos de diseño interno que no se publican.

---

## 12. Cómo construir y testear

### Requisitos

- Rust stable (edition 2024 — requiere Rust ≥ 1.85)
- PowerShell 7+ (para `run_tests.ps1` en Windows)
- Bash (para `run_tests.sh` en Linux/macOS)
- `@vscode/vsce` (`npm install -g @vscode/vsce`) para rebuildar la extensión

### Build

```powershell
cargo build           # debug
cargo build --release # release (usado por cargo-dist)
```

### Tests

```powershell
# Rust unit tests (lexer interno, etc.)
cargo test
```

Suite completa del lenguaje — Windows (PowerShell):

```powershell
.\run_tests.ps1                    # suite completa (E2E + unit + error + security)
.\run_tests.ps1 -unit              # solo unit tests (basados en framework.sz)
.\run_tests.ps1 -e2e               # E2E con golden files + tests de error
.\run_tests.ps1 -security          # solo tests de seguridad/error
.\run_tests.ps1 -filter "switch"   # tests cuyo nombre matchea un patrón
.\run_tests.ps1 -generate          # regenera los .expected tras cambiar el lenguaje
```

Linux / macOS (Bash) — mismos flags con doble guion:

```bash
./run_tests.sh                     # suite completa
./run_tests.sh --unit
./run_tests.sh --e2e
./run_tests.sh --security
./run_tests.sh --filter "switch"
./run_tests.sh --generate
```

⚠️ `-generate` / `--generate` sobrescribe los golden files: correlo solo cuando el
cambio de salida es intencional, y revisá el diff antes de commitear.

### Release local

Para generar el `.msi` localmente se requiere WiX Toolset v3 + `cargo install cargo-wix`. En la práctica el `.msi` se genera automáticamente vía GitHub Actions al hacer push de un tag.

### Extensión VS Code

```powershell
cd vscode-serez
vsce package                        # genera .vsix
antigravity-ide.cmd --install-extension serez-code-0.2.0.vsix
```

---

## 13. Convenciones para contribuir al core

### Invariantes del proyecto

- **Cero `unsafe` en el core del intérprete** — el modelo de memoria por arenas
  está construido a propósito sin bloques unsafe. Toda feature nueva mantiene esa
  invariante. (`namespaces_os.rs` usa `unsafe` solo para llamadas FFI de
  plataforma, tipo `GlobalMemoryStatusEx`.)
- **Dependencias de runtime mínimas** — agregar un crate nuevo exige una razón
  fuerte.
- **Los errores van a `stderr`** — `eprintln!` para todo error; `println!` solo
  para salida del programa (`out`) y el REPL.
- **Invariante de Flash Scope** — todo constructo nuevo a nivel bloque debe
  llamar `scopes.push()` antes de evaluar su cuerpo y `scopes.pop()` después, en
  **todos** los code paths incluidos los de error. Olvidar un pop en un camino de
  error deja el call stack sucio en el REPL.
- **Toda sintaxis nueva atraviesa el pipeline completo** — `token.rs` →
  `lexer.rs` → `ast.rs` → `parser.rs` → `evaluator/`. Nunca agregar al evaluador
  sin el nodo de AST correspondiente.

### Agregar un operador infijo

Requiere registrarlo en **dos** lugares de `parser.rs`, o el parser falla en
silencio:

```rust
// 1. token_precedence() — le da su binding power al operador
TokenType::MyOp => Precedence::Sum,

// 2. match is_infix — habilita a parse_expression a entrar al loop infijo
TokenType::MyOp => true,
```

Después, la evaluación va en `eval_infix()` (`evaluator/ops.rs`).

### Agregar un statement

1. Agregar la variante en `TokenType` (`token.rs`). Si es keyword, cablearla en `lookup_ident()`.
2. Agregar el/los nodo(s) de AST en `ast.rs`.
3. Agregar el handler de parseo en `parser.rs`, dentro de `parse_statement()`.
4. Agregar el handler de evaluación en `evaluator/stmt.rs`, dentro de `eval_statement()`.
5. Agregar un `.sz` de test que demuestre la feature.

### Pull requests

- Un cambio lógico por commit.
- Describir **por qué** se hizo el cambio, no solo qué cambió.
- Los PRs que agregan features del lenguaje incluyen al menos un `.sz` de ejemplo.

Las convenciones de issues y PRs están en [CONTRIBUTING.md](CONTRIBUTING.md).

---

## 14. Limitaciones conocidas del lenguaje

Comportamientos correctos pero que pueden sorprender:

### `for-in` crea copias

```sz
for (let x in arr) {
    x = x * 10;   // muta la copia — arr no cambia
}
// Fix: usar for (let i = 0; i < arr.length; i++) { arr[i] = ...; }
```

### `this.field[i].method()` no persiste

Acceder a `this.field` dentro de un método devuelve una copia. Los métodos encadenados sobre esa copia no escriben de vuelta a la instancia.

```sz
// ✅ Funciona: index-assign directo
this.items[0] = newValue;
// ✅ Funciona: método de mutación sobre this.field
this.items.push(val);
// ⚠️ No persiste: método encadenado sobre elemento
this.items[0].update(99);
```

### `{` en strings activa interpolación

```sz
out "empty: \{\}";   // ✅ → empty: {}
out "block: {";       // ❌ interpolación sin cerrar
```

### `\"` dentro de `{…}` rompe el parser

```sz
// ❌ Error de parser:
out "names: {arr.join(\", \")}";

// ✅ Extraer a variable:
let sep = ", ";
out "names: {arr.join(sep)}";
```

### Parámetros enum no deben anotarse como `string`

```sz
fn add(string priority) { ... }    // ❌ type error con Priority.High
fn add(priority) { ... }           // ✅
```

### `public abstract TYPE method()` no está soportado

```sz
// ❌ No soportado — error de parser
public abstract decimal area();

// ✅ Usar implementación por defecto que lanza
public decimal area() {
    throw "area() not implemented in " + this.name;
    return 0.0;
}
```

### Enum.Variant en `match` ✅ (arreglado en v2.1.0)

Antes era necesario capturar el enum en una variable y usar condicionales. Desde la corrección del parser (B-75 antecedente), los patrones `Enum.Variant` funcionan directamente:

```sz
match dir {
    case Direction.North => out "north";
    case Direction.South => out "south";
}
```

### Métodos de clase con nombre de keyword ✅ (arreglado en v2.1.0)

Antes, un método llamado `get`, `set` o `static` fallaba en el parser porque esos tokens son keywords (`KwGet`, `KwSet`, `KwStatic`). Desde B-75, el parser acepta cualquier token que no sea operador/delimitador como nombre de método:

```sz
class Counter {
    value: int = 0;
    public int get() { return this.value; }   // ✅ ahora funciona
    public void set(int v) { this.value = v; }  // ✅
}
```

---

## 15. Pendiente

### Features del lenguaje
- [ ] LSP server — diagnósticos en tiempo real en el editor (errores subrayados sin ejecutar)

### Tooling
- [ ] Formatter con espaciado de operadores (requiere tokenizer en el formatter)
- [ ] `sz --lint` — correr solo parser + type checker sin ejecutar (base para LSP)

### Release
- [ ] Publicar extensión VS Code en el marketplace
- [ ] Publicar `serez-code` en crates.io
- [ ] Subir `.vsix` como release asset en GitHub junto al `.msi`

### Seguridad del CI
- [ ] Pinear GitHub Actions a commit SHAs exactos (Dependabot lo hará automáticamente en el primer run semanal)

---

## 16. Apéndice — features implementadas (histórico)

Lista que vivía en el README bajo "Roadmap". Está enteramente en `[x]`: no es un
plan, es el registro de lo que ya existe, y se conserva acá porque es material de
contribuidor, no de usuario. El registro canónico y fechado de cambios es
[CHANGELOG.md](CHANGELOG.md) — ante cualquier discrepancia, manda el CHANGELOG.


### Language features
- [x] `&&` and `||` — logical AND and OR operators with short-circuit evaluation
- [x] `for` loop — `for (let i = 0; i < n; i++)`, nested loops, 1D/2D array traversal; update accepts `i++`, `i--`, `i += n`
- [x] Array mutation via index — `arr[i] = expr`, works in loops and from inside functions
- [x] String interpolation — `"Hello, {name}!"`, supports nested quotes inside `{…}` (e.g. `{dict["key"]}`)
- [x] Lexical closures — functions that capture variables from their defining scope
- [x] Native higher-order functions — `map`, `filter`, `reduce` with lambda syntax `x => expr` / `(x, i) => expr`
- [x] Array methods — `.push`, `.pop`, `.shift`, `.unshift`, `.remove`, `.reverse`, `.sort`, `.find`, `.findIndex`, `.indexOf`, `.includes`, `.every`, `.some`, `.slice`, `.flat`, `.join`
- [x] String methods — `.length`, `.substring`, `.slice`, `.split`, `.replace`, `.includes`, `.indexOf`, `.startsWith`, `.endsWith`, `.charAt`, `.trim`, `.trimStart` / `.trimLeft`, `.trimEnd` / `.trimRight`, `.toUpperCase`, `.toLowerCase`, `.padStart`, `.padEnd`, `.toString()`
- [x] Dict methods — `.toList()` (keys array), `.toArray()` (2D entries array); missing key returns `null`
- [x] `decimal` type — f64 literals (`3.14`), mixed arithmetic with `int`
- [x] Global conversions — `parseInt(val)`, `parseDecimal(val)`
- [x] Console input — `readLine(prompt?)`
- [x] Interfaces — typed record schemas: `interface Point { x: decimal, y: decimal }`, `new Point({ x:1.0, y:2.0 })`, field read/write, object patch `p = { x: 5.0 }`
- [x] Classes — C#-style OOP: `public class Foo`, constructor `public Foo(args)`, `this.field`, `public`/`private` methods, field assignment `obj.field = val`
- [x] Single inheritance — `public class Bar : Foo`, `super(args)` constructor delegation, `super.method()`, method override, inherited method lookup
- [x] Static methods — `public static T method(...)` on classes, called as `ClassName.method(args)`
- [x] Abstract classes — `abstract class Foo` cannot be instantiated; abstract methods have no body
- [x] Sealed classes — `sealed class Foo` cannot be subclassed
- [x] Getters / setters — `public get T prop()` / `public set prop(T val)` computed properties on class instances
- [x] `break` / `continue` — loop control flow inside `while`, `for`, `for-in`, and `do-while`
- [x] Labeled `break` / `continue` — `label: for ...` with `break label` / `continue label` for nested loop control
- [x] `do-while` loop — body executes at least once; `break`/`continue` supported
- [x] `switch` — `switch(expr) { case val: {} case a, b: {} default: {} }` — no fall-through
- [x] Exceptions — `try {} catch (e) {} finally {}` and `throw expr`; any value can be thrown
- [x] `const` — immutable variable declarations enforced at runtime
- [x] `enum` — `enum Color { Red, Green, Blue }` with `Color.Red` variant access
- [x] `Set` type — `new Set([...])`, methods: `add`, `has`, `delete`, `clear`, `size`, `toArray`, `union`, `intersection`
- [x] Null coalescing — `a ?? b` returns `a` if non-null, else evaluates `b`
- [x] Optional chaining — `a?.method()` / `a?.field` returns `null` without error when `a` is `null`; chains with `??`
- [x] Ternary operator — `cond ? then : else` with lazy evaluation and right-associativity
- [x] Escape sequences — `\n`, `\t`, `\r`, `\\`, `\"`, `\{` inside string literals
- [x] Block comments — `/* ... */` multi-line comments
- [x] Math namespace — `abs`, `sqrt`, `floor`, `ceil`, `round`, `trunc`, `min`, `max`, `pow`, `exp`, `log`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `clamp`, `sign`, `random`, `PI`, `E`
- [x] File namespace — `read`, `write`, `create`, `exists`, `read_asBinary`, `write_asBinary`
- [x] JSON namespace — `stringify`, `parse`, `pretty`
- [x] Power operator — `**` for integer and decimal exponentiation
- [x] Bitwise operators — `&`, `|`, `^`, `~`, `<<`, `>>` (64-bit signed integers); binary (`0b`) and hex (`0x`) literals; numeric separators (`1_000_000`)
- [x] `is` type-check operator — `expr is TypeName` returns `bool` at runtime
- [x] Default parameters — `fn int f(int x = 10)` with fallback when argument is omitted
- [x] Security test suite — 17 error tests (`sec_*.sz`) + 6 unit test files (`unit_sec_*.sz`) covering arithmetic, null safety, type safety, error isolation, injection, and resource limits
- [x] OS/hardware namespaces — `Terminal` (raw mode, keyboard, mouse, cursor), `OS` (platform, pid, exec, kill), `Env` (get, set, args), `Time` (now, sleep), `System` (cpuCount, totalMemory, freeMemory, hostname, uptime)
- [x] Socket namespace — TCP client/server (`connect`, `send`, `recv`, `listen`, `accept`, `close`) + RFC 6455 WebSocket text frames (`sendWsFrame`, `recvWsFrame`)
- [x] GPU namespace — CPU-backed compute buffers (`createBuffer`, `createBufferFromArray`, `map`, `reduce`, `dot`, `axpy`, `matmul`, `fill`, `readBuffer`, `freeBuffer`)
- [x] File extended — `listDir`, `mkdir`, `stat`, `delete`, `rename`
- [x] Permission system — three-level model: `serez.json` (project-wide) → `use permissions {}` (file-level) → `unsafe {}` (operation-level)
- [x] `use permissions {}` keyword — grants namespace access at file scope

### Type system
- [x] Typed arrays — `[int]`, `[string]`, `[decimal]`, `[T?]` with element-level enforcement on `push`, `unshift`, index-assign, and construction
- [x] Type inference for function call results — `let x = add(1, 2)` infers `x: int` in the static checker
- [x] Optional / nullable types — `int?`, `string?`, `fn int? search()`, `null` literal, null equality (`== null`, `!= null`)

### Tooling
- [x] Security test runner — `-security` / `--security` flag on `run_tests.ps1` / `run_tests.sh` runs all security test files
- [x] Cross-platform test runner — `run_tests.sh` (Bash) mirrors all flags of `run_tests.ps1` (PowerShell)
- [x] Span-aware error diagnostics — parser and runtime errors show the source line with a `^` caret
- [x] Watch mode — `sz --watch file.sz` re-runs on every save
- [x] VS Code extension — syntax highlighting and formatter for `.sz` files (`vscode-serez/`)
- [x] Demo apps — five `apps/*.sz` programs that exercise every language feature end-to-end
- [x] `.sz` file formatter — `DocumentFormattingEditProvider` integrado en la extensión VS Code; `formatOnSave` activado automáticamente para `.sz`
- [x] LSP server for editor support — `sz-lsp` binary (stdio JSON-RPC): live diagnostics (parser + type checker), completion (keywords, native namespaces + their methods, document symbols), hover, go-to-definition and document symbols; wired into the VS Code extension (`serez.lsp.enabled` / `serez.lsp.path`)
