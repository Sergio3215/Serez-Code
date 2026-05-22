# Serez-Code — Changelog

Registro técnico de todos los cambios al lenguaje, stdlib, y tooling.  
Orden: del más reciente al más antiguo.

---

## [Unreleased] — rama `improve`

### VS Code — Formatter (`vscode-serez` v0.2.0)

**`extension.js`** — nuevo `DocumentFormattingEditProvider`:
- Indentación automática con 4 espacios por nivel, basada en conteo de `{` y `}`
- Ignora llaves dentro de strings literales y comentarios de línea (`//`)
- `} else {` manejado correctamente: dedent antes de imprimir, indent después
- Colapsa líneas en blanco consecutivas a una sola
- Elimina trailing whitespace en todas las líneas
- El archivo siempre termina con exactamente un `\n`

**`package.json`** — versión `0.2.0`:
- `"main": "./extension.js"` y `"activationEvents": ["onLanguage:serez"]`
- Categoría `Formatters` añadida
- `configurationDefaults` para `.sz`: `editor.defaultFormatter` y `editor.formatOnSave: true` activados automáticamente

**Uso:** `Shift+Alt+F` para formatear manualmente, o guardar el archivo (formatOnSave).  
**Rebuild:** `vsce package` en `vscode-serez/` genera `serez-code-0.2.0.vsix`.

---

### CI / Tooling
- `release.yml`: permisos acotados por job — solo `host` tiene `contents: write`; los demás `contents: read`
- `.github/dependabot.yml`: actualizaciones semanales automáticas de GitHub Actions y dependencias Cargo
- `run_tests.sh`: script Bash equivalente a `run_tests.ps1`, con flags `--filter`, `--generate`, `--unit`, `--e2e`, `--security`; colores ANSI; normalización CRLF; archivos temporales únicos por proceso
- Evaluador refactorizado de un solo `evaluator.rs` (5300+ líneas) a 12 submódulos:

| Módulo | Responsabilidad |
|---|---|
| `mod.rs` | Entrada principal, Flash Scope protocol, StoredMethod cache, profiler estático |
| `stmt.rs` | Evaluación de statements (let, assign, for, while, return, …) |
| `expr.rs` | Evaluación de expresiones (calls, index, dot, ternary, …) |
| `ops.rs` | Operadores infix y prefix |
| `check.rs` | Helpers de type-check (parámetros, return, typed arrays) |
| `builtins.rs` | Funciones globales (parseInt, parseDecimal, readLine, …) |
| `classes.rs` | Instanciación, dispatch de métodos, herencia, super |
| `methods_array.rs` | Métodos de array (push, pop, map, filter, reduce, sort, …) |
| `methods_string.rs` | Métodos de string (split, replace, trim, padStart, …) |
| `methods_set.rs` | Métodos de Set (add, has, delete, toArray, union, …) |
| `namespaces.rs` | Namespaces builtin (Math, File, JSON) |
| `control.rs` | Helpers de control flow (break, continue, labeled loops, do-while) |

### Apps demo
- `apps/01_task_manager.sz` — enum, herencia, static methods, switch, HOF, try/catch
- `apps/02_statistics.sz` — typed arrays, Math, map/filter/reduce, Pearson correlation
- `apps/03_text_analyzer.sz` — string methods, dicts, Caesar cipher, File I/O
- `apps/04_bank_system.sz` — abstract class, sealed, interface, const, getters, optional chaining
- `apps/05_data_pipeline.sz` — JSON, File, Set, bitwise/power ops, pipeline HOF

---

## [0.1.0] — Historia del lenguaje

### Fase 5 — Corrección de bugs y semántica (B-62 a B-63)

**`reverse()` — mutación in-place con retorno (B-62)**
- Antes: `reverse()` devolvía void, no era encadenable
- Ahora: muta el array in-place Y devuelve el mismo array — permite `let sorted = arr.reverse()`

**`trimLeft` / `trimRight` como aliases (B-63)**
- Añadidos como aliases de `trimStart` / `trimEnd` para compatibilidad

---

### Fase 4 — Corrección de bugs críticos (B-54 a B-61)

**`is` operator — fix completo (B-61)**
- Bug: `is` era tokenizado como identificador, nunca funcionó como operador infix
- Fix: token `KwIs` añadido; registrado en `token_precedence()` y en el match `is_infix` del parser; handler `eval_infix` añadido en el evaluador
- `null is null` también corregido: faltaba el caso `("null", ObjectData::Null)` en `type_matches`

**Semántica de captura de funciones nombradas (B-58)**
- Antes: `fn` declarations capturaban el valor en el momento de definición (snapshot)
- Ahora: `fn` declarations usan semántica de referencia — rebind del slot global compartido
- Las lambdas mantienen semántica de snapshot (sin cambios)
- `ScopeStack::rebind()` añadido para rebinding selectivo de scope externo

**Mutación de dict desde scope anidado (B-57)**
- Bug: arena lifetime — la nueva entrada de un dict mutado desde dentro de una función quedaba en el scope local y era destruida al salir
- Fix: `plant_global` usado cuando `depth > 1`

**`padStart` / `padEnd` — early return incorrecto (B-56)**
- Bug: si la string ya tenía la longitud objetivo, retornaba vacío en lugar de retornar la string original
- Fix: early return corregido

**Validación de shift (B-55)**
- `1 << 64` y `8 >> -1` eran silenciosamente incorrectos
- Ahora son errores de runtime: shift negativo o ≥ 64 lanza error

**`flat(n)` — parámetro de profundidad (B-54)**
- Antes: solo soportaba `flat()` con profundidad 1
- Ahora: `flat(n)` aplana recursivamente `n` niveles; `flat()` equivale a `flat(1)`

**Getter-only — error al escribir (B-53)**
- Intentar asignar a una propiedad que solo tiene `get` (sin `set`) ahora es error de runtime

---

### Fase 3 — Nuevas funcionalidades del lenguaje

#### Operadores

**Operador power `**`**
- `2 ** 10` → `1024`; funciona con `int` y `decimal`
- Precedencia mayor que `*` / `/` / `%`
- `0 ** 0` → `1` (convención matemática)

**Operadores bitwise**
- `&` AND, `|` OR, `^` XOR, `~` NOT (prefix), `<<` shift izquierdo, `>>` shift aritmético derecho
- Solo para `int` (64-bit signed, two's complement)
- Shift negativo o ≥ 64 es error de runtime
- Literales binarios (`0b1010`) y hexadecimales (`0xFF`) soportados
- Separadores numéricos: `1_000_000`, `0xFF_FF`

**Encadenamiento opcional `?.`**
- `obj?.method()` / `obj?.field` — si `obj` es `null`, retorna `null` sin error
- Encadenable: `a?.getNext()?.getValue() ?? 0`
- Combinable con `??` para fallback

#### Control de flujo

**`do-while`**
- El cuerpo se ejecuta al menos una vez
- `break` y `continue` funcionan igual que en `while`/`for`

#### Clases

**Métodos estáticos**
- `public static T method(args)` en clases
- Llamados como `ClassName.method(args)` — no requieren instancia
- No tienen acceso a `this`

**Parámetros con valor por defecto**
- `fn int add(int a, int b = 10)` — si el caller omite el argumento, se usa el default
- El default es una expresión arbitraria evaluada en el momento del call
- El type checker maneja aridad variable (skip si hay defaults)

**Clases abstractas**
- `abstract class Foo` — no instanciable directamente; error de runtime en `new`
- Métodos sin cuerpo declarados para override en subclases

**Clases sealed**
- `sealed class Foo` — no heredable; intentar extenderla es error de runtime

**Getters y setters**
- `public get T prop()` — llamado automáticamente al leer `obj.prop` (sin paréntesis)
- `public set prop(T val)` — llamado automáticamente al asignar `obj.prop = val`
- Propiedad con solo getter es read-only; escribirla es error de runtime

**Campos de clase con valores por defecto**
- `field: type = value` en el cuerpo de la clase

#### Arrays — nuevos métodos

| Método | Descripción |
|---|---|
| `.find(cb)` | Primer elemento donde `cb` retorna `true`, o `null` |
| `.findIndex(cb)` | Índice del primer elemento que cumple el predicado, o `-1` |
| `.every(cb)` | `true` si `cb` es `true` para todos los elementos |
| `.some(cb)` | `true` si `cb` es `true` para al menos uno |
| `.slice(start, end)` | Nueva array desde `start` (inclusive) a `end` (exclusive) |
| `.flat(n?)` | Aplana `n` niveles de anidamiento (default 1) |
| `.reverse()` | Invierte in-place, retorna la misma array |
| `.indexOf(val)` | Índice de la primera ocurrencia, o `-1` |
| `.includes(val)` | `true` si la array contiene el valor |
| `.remove(idx)` | Elimina y retorna el elemento en `idx` |

#### Strings — nuevos métodos

| Método | Descripción |
|---|---|
| `.padStart(n, ch?)` | Rellena al inicio con `ch` (default espacio) hasta longitud `n` |
| `.padEnd(n, ch?)` | Rellena al final con `ch` (default espacio) hasta longitud `n` |
| `.slice(start, end?)` | Substring con soporte de índice negativo |
| `.trimStart()` / `.trimLeft()` | Elimina espacios al inicio |
| `.trimEnd()` / `.trimRight()` | Elimina espacios al final |
| `.toUpperCase()` / `.upper()` | Copia en mayúsculas |
| `.toLowerCase()` / `.lower()` | Copia en minúsculas |
| `.startsWith(prefix)` | `true` si la string empieza con `prefix` |
| `.endsWith(suffix)` | `true` si la string termina con `suffix` |
| `.charAt(i)` | Carácter en posición `i`, o `""` si fuera de rango |
| `.indexOf(sub)` | Índice de primera ocurrencia de `sub`, o `-1` |
| `.replace(from, to)` | Reemplaza **todas** las ocurrencias (antes solo la primera) |

---

### Fase 2 — Stdlib y tipos compuestos

#### `const`
- `const PI = 3.14159` — inmutable; cualquier reassign es error de runtime
- Mismo scoping que `let` — invisible fuera de su bloque

#### `enum`
- `enum Color { Red, Green, Blue }` — variantes accedidas como `Color.Red`
- Las variantes son su propio tipo (no `string`) — no anotar parámetros enum como `string`
- Comparables con `==` y usables en `switch case`
- Se muestran como `"Color.Red"` (nombre calificado completo)

#### Loops con etiquetas
- `outer: for (...)` + `break outer` / `continue outer`
- Funciona con `while`, `for`, `for-in`, `do-while`

#### Spread y rest
- Spread en array literals: `[...arr, 1, 2]`
- Spread en llamadas: `fn(...args)`
- Rest params: `fn void log(...args)` — `args` es array con todos los argumentos extra
- El type checker omite el check de aridad para funciones con rest params

#### Namespace `Math`

| Función/Constante | Descripción |
|---|---|
| `Math.PI`, `Math.E` | Constantes matemáticas |
| `Math.abs(x)` | Valor absoluto |
| `Math.floor(x)`, `Math.ceil(x)`, `Math.round(x)`, `Math.trunc(x)` | Redondeos (retornan `int`) |
| `Math.sqrt(x)` | Raíz cuadrada |
| `Math.pow(base, exp)` | Potencia |
| `Math.exp(x)`, `Math.log(x)`, `Math.log2(x)`, `Math.log10(x)` | Exponencial y logaritmos |
| `Math.sin(x)`, `Math.cos(x)`, `Math.tan(x)` | Trigonométricas (radianes) |
| `Math.asin(x)`, `Math.acos(x)`, `Math.atan(x)`, `Math.atan2(y, x)` | Trigonométricas inversas |
| `Math.min(a, b, ...)`, `Math.max(a, b, ...)` | Mínimo/máximo variádico |
| `Math.clamp(x, min, max)` | Clamp al rango `[min, max]` |
| `Math.sign(x)` | Retorna `1`, `0`, o `-1` |
| `Math.random()` | Decimal pseudo-aleatorio en `[0, 1)` (LCG) |

#### Namespace `File`

| Función | Descripción |
|---|---|
| `File.exists(path)` | `true` si el archivo existe |
| `File.read(path)` | Contenido del archivo como `string` |
| `File.write(path, content)` | Escribe/sobreescribe el archivo |
| `File.create(path)` | Crea archivo vacío si no existe (touch, idempotente) |
| `File.read_asBinary(path)` | Bytes del archivo como `[int]` (0–255 cada uno) |
| `File.write_asBinary(path, bytes)` | Escribe array de bytes al archivo |

#### Namespace `JSON`

| Función | Descripción |
|---|---|
| `JSON.stringify(value)` | Serializa cualquier valor a string JSON |
| `JSON.parse(string)` | Parsea un string JSON; error de runtime si es inválido |

#### Tipo `Set`

| Método/propiedad | Descripción |
|---|---|
| `new Set()`, `new Set([...])` | Crea set vacío o inicializado desde array (sin duplicados) |
| `.size` | Cantidad de elementos (propiedad, sin paréntesis) |
| `.add(val)` | Inserta `val` si no existe (muta in-place) |
| `.has(val)` / `.contains(val)` | `true` si el set contiene `val` |
| `.delete(val)` / `.remove(val)` | Elimina `val`, retorna `true` si existía |
| `.clear()` | Elimina todos los elementos |
| `.toArray()` | Retorna todos los elementos como array |
| `.union(other)` | Nuevo set con todos los elementos de ambos |
| `.intersection(other)` | Nuevo set con solo los elementos presentes en ambos |

---

### Fase 1 — Core del lenguaje

#### Variables y tipos
- `let x = value` — declaración; `x = value` — reasignación (sin `let`)
- Tipos primitivos: `int` (i64), `decimal` (f64), `bool`, `string`, `void`, `any`, `null`
- Tipos compuestos: array `[T]`, dict `<K,V>`, función, interfaz, instancia de clase
- Tipos nullable: `int?`, `string?` — aceptan el tipo base o `null`
- Typed arrays: `let nums [int] = [1, 2, 3]` — tipo enforceado en push, unshift, index-assign
- Inferencia de tipos: `let x = add(1, 2)` infiere `x: int` en el static checker

#### Operadores
- Aritméticos: `+`, `-`, `*`, `/` (entero, trunca), `%`
- Comparación: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Lógicos: `&&`, `||`, `!` (short-circuit)
- Ternario: `cond ? then : else` (lazy, right-associative)
- Null coalescing: `a ?? b`
- `is`: `expr is TypeName` — `true`/`false` en runtime
- Compound assignment: `+=`, `-=`, `*=`, `/=`, `%=`
- Increment/decrement: `++`, `--` (prefix y postfix, solo como statements)
- Repetición de string: `"ha" * 3` → `"hahaha"`
- Concatenación: `"x" + 42` → `"x42"`

#### Seguridad de runtime
- Overflow de enteros: `checked_*` — error en lugar de wrap silencioso
- División/módulo por cero: error de runtime
- Index fuera de rango: error de runtime
- Variable no declarada: error de runtime
- `return` fuera de función: error de runtime
- Stack overflow: error de runtime (no catchable vía try/catch)

#### Funciones
- Declaradas: `fn returnType name(type param) { ... }`
- Arrow: `let f = returnType (type param) => { ... }`
- Anónimas: `let f = fn void () { ... }`
- Primer clase: asignables a variables, pasables como argumentos
- Recursivas: soportadas con call stack en errores
- Closures léxicos: capturan variables del scope donde son definidas
- `fn` declarations: semántica de referencia (rebind del slot global)
- Lambdas (`x => expr`): semántica de snapshot (captura por valor)

#### Control de flujo
- `if` / `else if` / `else` — condición entre paréntesis, llaves obligatorias
- `while` — condición entre paréntesis
- `for` — `for (let i = 0; i < n; i++)` — update acepta `i++`, `i--`, `i+=n`, etc.
- `for-in` — `for (let x in arr)` itera array o string; `x` es una copia del elemento
- `break` / `continue` — en todos los loops
- `switch` — sin fall-through; `case a, b:` para múltiples valores; `default:`
- `try` / `catch(e)` / `finally` — `finally` siempre corre; `throw` acepta cualquier valor
- Bloques standalone `{ ... }` — crean nuevo Flash Scope

#### Arrays
- Literales: `[1, 2, 3]`, `[]`
- Index access: `arr[i]` (0-based)
- Index mutation: `arr[i] = val`
- Mutación global desde función: `data[i] = val` persiste; `this.arr[i] = val` persiste
- **Limitación**: `for-in` crea copia — mutar la variable del loop no afecta el array original
- Métodos de mutación: `.push`, `.pop`, `.shift`, `.unshift`, `.reverse`, `.sort`, `.sort("desc")`, `.sort((a,b) => ...)`
- Métodos de consulta: `.length`, `.join`, `.map`, `.filter`, `.reduce`

#### Strings
- Interpolación: `"Hola {name}!"` — soporta expresiones complejas dentro de `{}`
- `\{` para llave literal; `\"` dentro de `{...}` rompe el parser (usar variable)
- Escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\{`
- Métodos: `.length`, `.substring`, `.split`, `.replace`, `.includes`, `.trim`, `.toString()`

#### Diccionarios
- `let d <string,int> = ({"a",1},{"b",2})`
- Acceso: `d["key"]` — retorna `null` si la clave no existe (no error)
- Escritura: `d["key"] = val` o `d.Add({"key",val})`
- Métodos: `.Add`, `.Remove`, `.RemoveAll`, `.clear`, `.toList`, `.toArray`

#### Clases e interfaces
- `interface Point { x: decimal, y: decimal }` — record de campos tipados, sin métodos
- `class Foo { public Foo(args) { ... } }` — constructor + campos + métodos
- Herencia simple: `class Bar : Foo { ... }`, `super(args)` en constructor
- `public` / `private` — `private` solo accesible desde métodos de la misma clase
- Instancia: `let obj = new Foo(args)`
- Mutación de campo: `obj.field = val`
- **Limitación**: `this.field[i].method()` dentro de un método de clase crea una copia — el resultado no persiste; usar `this.field[i] = newValue` en su lugar

#### Conversiones y I/O
- `parseInt(val)` — convierte a `int` (string, decimal, int)
- `parseDecimal(val)` — convierte a `decimal` (string, int, decimal)
- `readLine(prompt?)` — lee una línea de stdin
- `out expr` — imprime a stdout con newline; statement, no función

#### Memoria — Flash Scopes
- Dos arenas: global (todo el programa) y scoped (local por bloque)
- Cada `{ }` graba un watermark al entrar y trunca al salir — O(k) por scope
- Valores de retorno extraídos como `OwnedValue` antes del pop y replantados en el scope padre
- `Rc<BlockStatement>` para cuerpos de función — clonar una función es O(1)
- `StoredMethod` en clases — dispatch O(1) sin clonar el body del método

#### Tooling
- `sz script.sz` — ejecutar archivo
- `sz` — REPL
- `sz --check script.sz` — profiler estático (estimación de bytes por función)
- `sz --watch script.sz` — reruns automático al guardar
- `sz --version` — versión
- Errores con span: línea + columna + caret `^` en el source
- VS Code extension: syntax highlighting para `.sz`
