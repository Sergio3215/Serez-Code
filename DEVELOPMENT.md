# Serez-Code — Development Reference

Documentación completa del proyecto: arquitectura, decisiones técnicas, tooling, CI/CD, y estado actual.

---

## Índice

1. [Estado del proyecto](#1-estado-del-proyecto)
2. [Estructura del repositorio](#2-estructura-del-repositorio)
3. [Arquitectura del intérprete](#3-arquitectura-del-intérprete)
4. [Pipeline de ejecución](#4-pipeline-de-ejecución)
5. [Modelo de memoria — Flash Scopes](#5-modelo-de-memoria--flash-scopes)
6. [Evaluador — submódulos](#6-evaluador--submódulos)
7. [Suite de tests](#7-suite-de-tests)
8. [Apps demo](#8-apps-demo)
9. [Extensión VS Code](#9-extensión-vs-code)
10. [CI/CD — Release pipeline](#10-cicd--release-pipeline)
11. [Seguridad del repositorio](#11-seguridad-del-repositorio)
12. [Cómo construir y testear](#12-cómo-construir-y-testear)
13. [Limitaciones conocidas del lenguaje](#13-limitaciones-conocidas-del-lenguaje)
14. [Pendiente](#14-pendiente)

---

## 1. Estado del proyecto

| Métrica | Valor |
|---|---|
| Versión | 2.1.0 |
| Tests pasando | 249 (0 fallando) |
| Archivos Rust | 28 (`src/`) |
| Tamaño del parser | ~100 KB |
| Tamaño del evaluador (total submodulos) | ~320 KB |
| Extensión VS Code | v0.2.0 |
| Plataformas de release | Windows (MSI), Linux (shell), macOS (shell/PS) |

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
│   └── evaluator/              — Intérprete tree-walking (18 submódulos)
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
│   ├── unit_*.sz               — 95 tests unitarios
│   ├── sec_*.sz + err_*.sz     — 70 tests de seguridad y error
│   ├── demo_*.sz               — 3 demos
│   └── NN_*.sz + *.expected    — 68 tests E2E con golden files
│
├── apps/                       — 5 apps demo (ejercitan todo el lenguaje)
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
├── README.md                   — Documentación del lenguaje (referencia completa)
├── CHANGELOG.md                — Historial técnico de cambios por fase
├── DEVELOPMENT.md              — Este archivo
└── bugs.md                     — Log de 63 bugs documentados (todos corregidos)
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
Evaluator (evaluator/ — 17 módulos)
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

## 5. Modelo de memoria — Flash Scopes

### Dos arenas

```
Global Arena
  — Variables top-level, funciones, clases
  — Persiste toda la vida del programa
  — Scratch watermark: temporales de `out` liberados después de cada statement

Scoped Arena
  — Variables locales, argumentos, temporales de bloque
  — Stack de watermarks: una entrada por scope activo
  — Cleanup: Vec::truncate(watermark) — O(k) drops, sin GC
```

### ObjectRef

Cada valor en el intérprete es una referencia segura:

```rust
ObjectRef { region: RegionId, index: usize }
```

- `region`: Global o Scoped — determina qué arena leer
- `index`: posición dentro del Vec de la arena
- No puede dangling: al truncar la arena el índice queda inaccesible, no apunta a memoria inválida

### Protocolo de scope (invariante "promote before pop")

Todo bloque `{ }` sigue esta secuencia en TODOS los code paths incluyendo errores:

```
1. scopes.push()               — graba watermark
2. evaluar statements del bloque
3. extract(result_ref)         — deep clone a OwnedValue (arena-independent)
4. scopes.pop()                — trunca arena: libera todos los locales
5. plant(owned)                — re-alloca en el scope padre
```

### Optimizaciones de arena

| Colección | Capacidad inicial |
|---|---|
| Global arena | 256 objetos |
| Scoped arena | 64 objetos |
| Frame de scope | 4 entradas |
| `global_bindings` | 32 entradas |
| Registry de interfaces/clases | 8 entradas c/u |

---

## 6. Evaluador — submódulos

El evaluador original era un solo archivo de 5300+ líneas. Fue dividido en 17 módulos cohesivos:

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
| `leave_call()` | `scopes.pop(); call_depth -= 1; call_stack.pop()` — 11 sitios |
| `print_call_stack()` | Loop de 3 líneas para imprimir la cadena de calls — 6 sitios |
| `plant_for_target(value, ref)` | Selección region-aware de arena para dict IndexAssign — 3 sitios |

---

## 7. Suite de tests

### Estructura

| Categoría | Cantidad | Descripción |
|---|---|---|
| `unit_*.sz` (no sec) | 73 | Tests unitarios usando `framework.sz` (assert, expect) |
| `NN_*.sz` + `.expected` | 55 | Tests E2E con golden files — diff exacto de stdout |
| `err_*.sz` | 27 | Verifican que ciertos inputs producen error de runtime |
| `sec_*.sz` | 36 | Suite de seguridad: overflow, OOB, null safety, stack overflow |
| `unit_sec_*.sz` | 15 | Tests unitarios de seguridad (con framework.sz) |
| CLI / REPL / --check | 13 | Tests de modo de ejecución del CLI |
| `framework.sz` | 1 | Framework compartido por todos los unit tests |
| **Total** | **256** | **0 fallando** |

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

# Suite completa del lenguaje
.\run_tests.ps1       # Windows
./run_tests.sh        # Linux/macOS
```

### Release local

Para generar el `.msi` localmente se requiere WiX Toolset v3 + `cargo install cargo-wix`. En la práctica el `.msi` se genera automáticamente vía GitHub Actions al hacer push de un tag.

### Extensión VS Code

```powershell
cd vscode-serez
vsce package                        # genera .vsix
antigravity-ide.cmd --install-extension serez-code-0.2.0.vsix
```

---

## 13. Limitaciones conocidas del lenguaje

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

---

## 14. Pendiente

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
