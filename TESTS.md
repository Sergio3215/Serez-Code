# Serez-Code — Documentación de Tests

## Cómo ejecutar

```powershell
.\run_tests.ps1                    # suite completa
.\run_tests.ps1 -unit              # solo unit tests
.\run_tests.ps1 -e2e               # solo E2E + error tests
.\run_tests.ps1 -filter "switch"   # filtra por nombre
.\run_tests.ps1 -generate          # regenera .expected (tras cambios en el lenguaje)
```

**Resultado:** 91 archivos · 162 casos unit · 0 fallos

---

## Tests E2E (Golden File)

Cada archivo `tests/NN_*.sz` se ejecuta y su `stdout` se compara contra `tests/NN_*.expected`.
Un fallo indica que el output cambió respecto al baseline guardado.

---

### `01_basic.sz` — Tipos primitivos y operadores básicos
Verifica que todos los tipos primitivos se evalúan y muestran correctamente.

| # | Qué verifica |
|---|--------------|
| 1 | Aritmética entera: `+`, `-`, `*`, `/`, `%` |
| 2 | Aritmética decimal: suma, multiplicación, división |
| 3 | Booleanos: `true`, `false`, `!true`, `!false` |
| 4 | Strings: literal y concatenación con `+` |
| 5 | Comparaciones: `==`, `!=`, `>`, `<`, `>=`, `<=` |
| 6 | `null` literal |

---

### `01_arithmetic.sz` — Aritmética básica
Aritmética de enteros y decimales, precedencia de operadores, y `null` coalescing `??`.

| # | Qué verifica |
|---|--------------|
| 1 | Suma, resta, multiplicación, división entera, módulo |
| 2 | Decimales: operaciones con punto flotante |
| 3 | Operaciones mixtas `int + decimal` |
| 4 | Precedencia: `*` antes de `+` |
| 5 | `null ?? valor_por_defecto` |

---

### `01_variables.sz` — Declaración y tipos de variables
Declaración con `let`, reasignación, y todos los tipos de dato.

| # | Qué verifica |
|---|--------------|
| 1 | `let` con cada tipo primitivo: `int`, `decimal`, `string`, `bool`, `null` |
| 2 | Reasignación de variables |
| 3 | Nombres de variable largos |

---

### `02_arithmetic.sz` — Aritmética avanzada
Profundiza en casos de borde aritméticos.

| # | Qué verifica |
|---|--------------|
| 1 | División entera trunca (no devuelve decimal) |
| 2 | Negación unaria en enteros y decimales |
| 3 | Operaciones mixtas `int` / `decimal` |
| 4 | Detección de overflow de entero |
| 5 | Repetición de string con `*` |
| 6 | Precedencia compleja con paréntesis |

---

### `02_variables.sz` — Variables y tipos
Variables con distintos tipos, reasignación y conversión.

| # | Qué verifica |
|---|--------------|
| 1 | Declaración de todos los tipos primitivos |
| 2 | Reasignación cambia el valor correctamente |
| 3 | `null` coalescing `??` sobre distintos tipos |

---

### `02_variables_scope.sz` — Scoping de variables
Comportamiento de scope en bloques y funciones.

| # | Qué verifica |
|---|--------------|
| 1 | Variable de bloque no escapa al outer scope |
| 2 | Función puede modificar variable outer |
| 3 | Shadowing de variable dentro de función |
| 4 | `null` en scope anidado |

---

### `03_control_flow.sz` — Flujo de control básico
`if/else`, `while`, `for`, `break`, `continue`.

| # | Qué verifica |
|---|--------------|
| 1 | `if`/`else` simple |
| 2 | `while` con acumulador |
| 3 | `for` con índice |
| 4 | `break` sale del bucle |
| 5 | `continue` salta la iteración |

---

### `03_strings.sz` — Métodos de string
Todos los métodos built-in de string.

| # | Qué verifica |
|---|--------------|
| 1 | `.length()` |
| 2 | `.includes()` / `.contains()` |
| 3 | `.replace()` / `.replaceAll()` |
| 4 | `.split()` |
| 5 | `.substring()` |
| 6 | `.toString()` en números |
| 7 | Interpolación `"{expr}"` |

---

### `04_control_flow.sz` — Flujo de control completo
Control de flujo más avanzado con condiciones compuestas y bucles anidados.

| # | Qué verifica |
|---|--------------|
| 1 | Cadena `if`/`else if`/`else` |
| 2 | Condiciones compuestas con `&&`, `\|\|` |
| 3 | `while` con `break` y `continue` |
| 4 | `for` con `break` anticipado |
| 5 | Bucles anidados |
| 6 | `if` como expresión (valor de retorno) |

---

### `04_functions.sz` — Funciones básicas y recursión
Declaración, retorno, parámetros, recursión, y funciones de orden superior.

| # | Qué verifica |
|---|--------------|
| 1 | `fn` con tipo de retorno y parámetros |
| 2 | Recursión: factorial, fibonacci |
| 3 | Funciones como valores (asignadas a variables) |
| 4 | Closures que capturan el entorno |
| 5 | Funciones de orden superior (`any f`) |

---

### `05_arrays.sz` — Arrays básicos
Arrays tipados y sus operaciones fundamentales.

| # | Qué verifica |
|---|--------------|
| 1 | Declaración `[int]`, `[string]` |
| 2 | Acceso por índice, mutación |
| 3 | `.push()`, `.pop()`, `.shift()`, `.unshift()` |
| 4 | `.sort()` ascendente y descendente |
| 5 | `.map()`, `.filter()`, `.reduce()` |
| 6 | Encadenamiento de métodos |

---

### `05_functions.sz` — Funciones avanzadas
Tipos de retorno explícitos, lambdas, currying y funciones anidadas.

| # | Qué verifica |
|---|--------------|
| 1 | Funciones con firma completa de tipos |
| 2 | Funciones literales (lambdas con `=>`) |
| 3 | Funciones como argumentos (`any`) |
| 4 | Currying y composición |
| 5 | Funciones que devuelven funciones |

---

### `06_arrays.sz` — Arrays avanzados
Operaciones de mutación, sort con comparador, y tipado estricto.

| # | Qué verifica |
|---|--------------|
| 1 | Mutación por índice `arr[i] = v` |
| 2 | `.pop()` / `.shift()` devuelven el valor removido |
| 3 | `.sort()` con comparador lambda `(a, b) => a - b` |
| 4 | Tipado estricto rechaza push de tipo incorrecto |
| 5 | Encadenamiento `filter().map().reduce()` |

---

### `06_strings.sz` — Strings avanzados
Acceso a propiedades, interpolación con expresiones complejas, y métodos.

| # | Qué verifica |
|---|--------------|
| 1 | `.length` como propiedad y `.length()` como método |
| 2 | Interpolación con expresiones, llamadas a funciones |
| 3 | Concatenación de distintos tipos |
| 4 | `.split()` y acceso al resultado |
| 5 | String vacío y sus métodos |

---

### `07_dicts.sz` — Diccionarios
Creación, acceso, mutación y métodos de diccionarios tipados.

| # | Qué verifica |
|---|--------------|
| 1 | Declaración `<string, int>` con pares iniciales |
| 2 | Acceso por clave `dict["clave"]` |
| 3 | Modificación de valor existente |
| 4 | `.Add()` para insertar nuevos pares |
| 5 | `.Remove()` para eliminar por clave |
| 6 | `.toList()` / `.toArray()` |
| 7 | Dict con valores `any` para tipos mixtos |

---

### `08_classes.sz` — Clases e instancias
Definición de clases, constructores, métodos, herencia y polimorfismo.

| # | Qué verifica |
|---|--------------|
| 1 | Constructor `public Clase(params)` |
| 2 | `this.campo = valor` en constructor |
| 3 | Llamadas a métodos de instancia |
| 4 | Herencia: `class B extends A` |
| 5 | `super(args)` en constructor hijo |
| 6 | Polimorfismo: método sobreescrito |
| 7 | Cálculos matemáticos dentro de métodos |

---

### `09_interfaces.sz` — Interfaces
Definición de interfaces, instanciación y patching de objetos.

| # | Qué verifica |
|---|--------------|
| 1 | `interface I { tipo campo; }` |
| 2 | `new I { campo: valor }` |
| 3 | Acceso y modificación de campos |
| 4 | Patching completo y parcial con `{ campo: nuevo }` |
| 5 | Arrays de interfaces con `.filter()` |

---

### `10_lambdas.sz` — Lambdas y funciones de orden superior
Sintaxis lambda, closures, `map`/`filter`/`reduce`, composición.

| # | Qué verifica |
|---|--------------|
| 1 | Lambda de un parámetro: `x => x * x` |
| 2 | Lambda de dos parámetros: `(a, b) => a + b` |
| 3 | Lambda con cuerpo bloque: `(a, b) => { ... }` |
| 4 | `.map()`, `.filter()`, `.reduce()` con lambdas |
| 5 | `.sort()` con comparador |
| 6 | Closure captura variable del entorno |
| 7 | HOF propias (`mi_map`, `mi_filter`) |
| 8 | Encadenamiento `filter().map().filter()` |
| 9 | Lambda con índice: `(item, i) => ...` |
| 10 | Composición: `componer(f, g)` |

---

### `11_nullables.sz` — Nullables y null coalescing
Manejo de `null`, tipos nullable `T?`, y operador `??`.

| # | Qué verifica |
|---|--------------|
| 1 | `null == null`, `null != null` |
| 2 | `null ?? "por defecto"` con distintos tipos |
| 3 | Cadena de `??`: `a ?? b ?? c` |
| 4 | Función con retorno `string?` |
| 5 | `if (valor == null)` en condición |
| 6 | Array con nulls filtrado con `.filter(x => x != null)` |
| 7 | `null ??` con expresión compleja como fallback |

---

### `12_math.sz` — Funciones matemáticas
Todas las funciones `Math.*` built-in.

| # | Qué verifica |
|---|--------------|
| 1 | `Math.abs()` en int y decimal |
| 2 | `Math.sqrt()` |
| 3 | `Math.floor()`, `Math.ceil()`, `Math.round()` |
| 4 | `Math.min()`, `Math.max()` con int y decimal mixtos |
| 5 | `Math.pow()` |
| 6 | `Math.log()`, `Math.log2()`, `Math.log10()` |
| 7 | Fibonacci con Math para demostración |

---

### `13_edge_cases.sz` — Casos extremos generales
17 escenarios de borde que cruzan varias features.

| # | Qué verifica |
|---|--------------|
| 1 | String vacío: `""`, `.length()`, comparación |
| 2 | Array de un elemento: acceso, push |
| 3 | Función sin argumentos |
| 4 | `return` en medio de `for` |
| 5 | Closure make_adder con valores distintos |
| 6 | Interpolación con llamada a función |
| 7 | Recursión con acumulador (`suma_hasta`) |
| 8 | Clase con constructor, getter y mutación |
| 9 | Comparación entre distintos tipos (`1==1`, `"a"=="a"`, `null==null`) |
| 10 | `??` sobre resultado de función nullable |
| 11 | Función que recibe y devuelve array |
| 12 | Encadenamiento de métodos de string |
| 13 | Entero máximo `i64` |
| 14 | `if/else if` anidado profundo |
| 15 | Array de funciones lambda |
| 16 | Boolean equality (fix B-xx) |
| 17 | Módulo mixto `int % decimal`, `decimal % int` |

---

### `14_arch_features.sz` — Features arquitecturales
Features que afectan el diseño del evaluador.

| # | Qué verifica |
|---|--------------|
| 1 | `.length` como propiedad (sin paréntesis) |
| 2 | Secuencias de escape en strings |
| 3 | Mutación de campo de instancia desde función externa |
| 4 | Patching de objeto de interface |
| 5 | Herencia de 3 niveles (`A → B → C`) |
| 6 | `break` en bucle anidado (rompe el bucle correcto) |
| 7 | Short-circuit `&&` y `\|\|` |
| 8 | `return` desde bucle anidado en función |
| 9 | Closures en bucles capturando variable de iteración |
| 10 | Mutación de dict global desde función |

---

### `15_arch_stress.sz` — Estrés arquitectural
Casos que combinan múltiples features a la vez.

| # | Qué verifica |
|---|--------------|
| 1 | `.sort()` con comparadores numéricos y de string |
| 2 | Array tipado rechaza push de tipo incorrecto |
| 3 | Clase con campo array, métodos que lo manipulan |
| 4 | Pipeline dict: `filter` + `map` + `reduce` |
| 5 | Herencia + override de método |
| 6 | Composición de closures |
| 7 | Recursión mutua (dos funciones que se llaman entre sí) |
| 8 | Interpolación con expresiones complejas |
| 9 | Función que devuelve array de instancias |
| 10 | `continue` dentro de bucle con lógica compleja |

---

### `16_error_paths.sz` — Caminos de error controlados
Comportamientos que antes podían fallar silenciosamente.

| # | Qué verifica |
|---|--------------|
| 1 | Repetición de string con `*` |
| 2 | Concatenación mixta string + distintos tipos |
| 3 | `.unshift()` agrega al frente |
| 4 | Asignación directa a clave de dict |
| 5 | Array nullable `[string?]` |
| 6 | Modificación de array global desde función |
| 7 | `.sort()` con flag de dirección |

---

### `17_function_syntax.sz` — Variantes de sintaxis de funciones
Todas las formas de definir y usar funciones.

| # | Qué verifica |
|---|--------------|
| 1 | Arrow function con tipo de retorno explícito |
| 2 | Función anónima asignada a variable |
| 3 | Función como valor pasada a otra función |
| 4 | Composición y currying |
| 5 | Lambda de un parámetro sin paréntesis |
| 6 | Lambda con cuerpo multi-línea |
| 7 | Array de funciones |
| 8 | Parámetros sin tipo (`any`) |

---

### `18_error_cases.sz` — Comportamientos límite de operadores
Casos de borde que no producen error pero sí comportamiento específico.

| # | Qué verifica |
|---|--------------|
| 1 | `null ??` en variantes de tipos |
| 2 | Precedencia de operadores |
| 3 | Short-circuit con efectos secundarios |
| 4 | Negación `!` sobre resultado de comparación |
| 5 | Comparaciones cruzadas de tipos |
| 6 | Encadenamiento de métodos de string |
| 7 | `parseInt()`, `parseDecimal()` |
| 8 | Mutación de array por referencia |
| 9 | `.pop()` / `.shift()` devuelven el elemento |
| 10 | `.toString()` en primitivos |

---

### `19_untested_docs.sz` — Features documentadas no testeadas
Features que existían en docs pero no tenían test.

| # | Qué verifica |
|---|--------------|
| 1 | `.reduce()` con acumulador string |
| 2 | `filter` + `reduce` encadenados |
| 3 | `dict.toArray()` con filtrado |
| 4 | `parseInt()` con espacios en blanco |
| 5 | `replace()` vs `replaceAll()` (reemplaza primero vs todos) |
| 6 | `.split("")` con separador vacío |
| 7 | `.sort()` con flag de dirección explícito |
| 8 | `.map()` con parámetro de índice |
| 9 | Bloque standalone `{ ... }` con scoping |
| 10 | Closure capturando variables externas |
| 11 | `.toString()` en distintos tipos |
| 12 | `.contains()` como alias de `.includes()` |

---

### `20_more_edge_cases.sz` — Más casos extremos
Combinaciones de features en escenarios prácticos.

| # | Qué verifica |
|---|--------------|
| 1 | `arr.length` en interpolación |
| 2 | Llamada a método dentro de interpolación |
| 3 | Asignación a clave de dict |
| 4 | Encadenamiento de métodos |
| 5 | Función pasada como valor |
| 6 | `if` anidado como expresión |
| 7 | `return` anticipado en `for` |
| 8 | Array creado dentro de función |
| 9 | Uso del valor de retorno de función |

---

### `21_string_interp_complex.sz` — Interpolación compleja
Interpolación `"{expr}"` con expresiones no triviales.

| # | Qué verifica |
|---|--------------|
| 1 | Acceso a dict con clave entre comillas dentro de `{}` |
| 2 | `arr[i]` dentro de interpolación |
| 3 | Llamada a método dentro de interpolación |
| 4 | Expresión aritmética en interpolación |
| 5 | Campo de instancia de clase en interpolación |
| 6 | `null ??` dentro de interpolación |

---

### `22_math_edge.sz` — Casos extremos matemáticos
Comportamientos específicos de las funciones matemáticas y conversión numérica.

| # | Qué verifica |
|---|--------------|
| 1 | `Math.abs()` con positivo, negativo y cero |
| 2 | `Math.sqrt()` exacto e irracional |
| 3 | `Math.floor()`, `Math.ceil()`, `Math.round()` en valores medios |
| 4 | `Math.min()` / `Math.max()` con mixtos |
| 5 | `Math.pow()` con base y exponente entero y decimal |
| 6 | División entera trunca hacia cero |
| 7 | Display de decimal: trailing zeros y `d.0` |
| 8 | Módulo con negativos |

---

### `23_boundary_cases.sz` — Casos límite de tipos y estructuras
Límites de arrays, strings y dicts en condiciones extremas.

| # | Qué verifica |
|---|--------------|
| 1 | Repetición de string con factor `0` → string vacío |
| 2 | `.sort()` en array vacío (no falla) |
| 3 | `.split("")` en string vacío |
| 4 | `dict.Remove()` de clave inexistente (no falla) |
| 5 | Cadena de `??` cuando todos son null |
| 6 | Comparaciones booleanas |
| 7 | Precisión decimal con `0.1 + 0.2` |
| 8 | Negativos decimales |
| 9 | `parseInt()` aplicado a decimal |
| 10 | `parseDecimal()` aplicado a entero |

---

### `24_chained_calls.sz` — Llamadas encadenadas
Encadenamiento de métodos en arrays, strings y clases.

| # | Qué verifica |
|---|--------------|
| 1 | `arr.sort().map()` encadenado |
| 2 | Métodos de string encadenados |
| 3 | Resultado de método usado directamente en expresión |
| 4 | Builder pattern en clase (métodos retornan `this` implícitamente) |
| 5 | Función que retorna instancia de clase |

---

### `26_complex_scenarios.sz` — Escenarios complejos
Escenarios que integran múltiples features del lenguaje.

| # | Qué verifica |
|---|--------------|
| 1 | Array 2D: acceso `arr[i][j]` |
| 2 | Recorrido de array 2D con bucle anidado |
| 3 | Variable global modificada desde función anidada |
| 4 | `return` desde `if` dentro de `while` |
| 5 | Dict con valores `any` (tipos mixtos) |
| 6 | Array de instancias de clase |
| 7 | Múltiples closures capturando valores diferentes |

---

### `27_escape_sequences.sz` — Secuencias de escape
Verificación de todas las secuencias de escape en strings.

| Secuencia | Verifica |
|-----------|---------|
| `\n` | Salto de línea |
| `\t` | Tabulación |
| `\"` | Comilla doble literal |
| `\\` | Barra invertida literal |
| `\{` | Llave literal (sin interpolación) |
| `\r` | Retorno de carro |

---

### `28_final_checks.sz` — Verificaciones finales
Comportamientos adicionales de dicts, funciones y clases.

| # | Qué verifica |
|---|--------------|
| 1 | Dict preserva orden de inserción |
| 2 | `.toList()` y `.toArray()` |
| 3 | Múltiples `return` en distintas ramas de función |
| 4 | Función nullable devuelve `null` o valor |
| 5 | Función que llama a otra función |
| 6 | Encadenamiento de métodos con operaciones de string |

---

### `29_bug_regression.sz` — Regresiones de bugs (B-30, B-31, B-35, B-36, B-39, B-41, B-42)
Tests añadidos específicamente para cada bug corregido.

| Bug | Qué verifica |
|-----|--------------|
| B-35 | `for (let i = arr[0]; ...)` no corrompe `arr[0]` |
| B-36 | Negación de negativo: `-(-1)` = `1`; valores grandes sin overflow |
| B-39 | `"str" + decimal` usa el mismo formato que `out decimal` |
| B-41 | `.remove(idx)` devuelve el elemento y acorta el array |
| B-42 | `.trim()`, `.toUpperCase()`, `.toLowerCase()`, `.upper()`, `.lower()`, `.startsWith()`, `.endsWith()`, `.indexOf()`, `.charAt()` |
| B-30 | `.pop()` / `.shift()` en array vacío devuelven `null` |
| B-31 | `dict["claveInexistente"]` devuelve `null` |
| B-03/36 | Aritmética normal dentro del rango no falla |

---

### `30_class_regression.sz` — Regresiones de bugs en clases (B-28, B-29, B-32, B-34, B-40, B-41)
Tests que verifican correcciones de bugs específicos en el sistema de clases.

| Bug | Qué verifica |
|-----|--------------|
| B-29 | Método de clase puede devolver `[int]` (array tipado) |
| B-28 | `this.campo[idx] = valor` funciona dentro de método |
| B-32 | `.sort()`, `.shift()`, `.unshift()` sobre campos de instancia |
| B-34 | Campo que almacena función puede llamarse: `this.fn()` |
| B-40 | Call stack rastreo correcto en métodos (profundidad) |
| B-41 | `.remove()` sobre campo array de instancia |

---

### `31_compound_assign.sz` — Operadores de asignación compuesta (E2E)
Cobertura básica E2E de `+=`, `-=`, `*=`, `/=`, `%=`.

| # | Qué verifica |
|---|--------------|
| 1 | `+=` en entero |
| 2 | `-=` en entero |
| 3 | `*=` en entero |
| 4 | `/=` en entero |
| 5 | `%=` en entero |
| 6 | `+=` en string (concatena) |
| 7 | `+=` en decimal |
| 8 | `+=` en acumulador de bucle |
| 9 | `+=` en elemento de array |
| 10 | `*=` en elemento de array |
| 11 | `+=` en campo de instancia (vía método) |

---

### `32_switch.sz` — Switch (E2E)
Cobertura básica E2E del `switch`.

| # | Qué verifica |
|---|--------------|
| 1 | Match exacto de entero |
| 2 | Case con múltiples valores: `case 1, 2, 3:` |
| 3 | Match de string |
| 4 | `default` cuando ningún case coincide |
| 5 | `switch` dentro de función con `return` |
| 6 | Switch con expresión como valor: `arr[i] / 10` |

---

### `33_try_catch.sz` — Try / Catch / Throw / Finally (E2E)
Cobertura E2E completa del manejo de excepciones.

| # | Qué verifica |
|---|--------------|
| 1 | `catch` captura string lanzado |
| 2 | `throw` con entero |
| 3 | `finally` corre aunque haya throw |
| 4 | `finally` corre sin throw (path normal) |
| 5 | Excepción lanzada desde función propagada al caller |
| 6 | Función sin error: no dispara catch |
| 7 | Try anidado: inner catch, outer no ve la excepción |
| 8 | `finally` dentro de función con `return` en catch |
| 9 | Excepción desde método de clase (`BankAccount.withdraw`) |
| 10 | Balance no cambia si el withdraw falla |

---

### `38_real_programs.sz` — Programas reales E2E (8 programas completos)
Integración completa del lenguaje: 8 programas reales que ejercen todas las características implementadas.

| # | Programa | Qué verifica |
|---|----------|--------------|
| 1 | Bank Account | Clases, getters, excepciones, optional chaining `?.`, `??` |
| 2 | Task Manager | Enums (`Priority`, `TaskStatus`), `Set` (deduplicación), métodos estáticos, factory |
| 3 | Shape Hierarchy | Clases `abstract`/`sealed`, herencia, `Math.PI`, `Math.round` |
| 4 | Functional Pipeline | Closures, `compose`, spread `...`, rest `...params`, `map`/`filter`/`reduce` |
| 5 | JSON Config | `JSON.stringify` y `JSON.parse` con primitivos, arrays y roundtrip |
| 6 | Algorithms | `factorial`, `fib`, bitwise `is_power_of_two`, `count_bits`, Newton `sqrt` |
| 7 | Error Handling | Jerarquía `AppError`/`NetworkError`, `is` type dispatch, `finally` |
| 8 | String Processing | `padStart`, `trimLeft`/`trimRight`, `split`, `slice`, `toUpperCase` |

---

## Tests de Error (`err_*`)

Cada archivo `tests/err_*.sz` debe producir al menos una línea `❌` en stderr.
Si no hay error, el test **falla** (la condición de error no fue detectada).

| Archivo | Condición de error que verifica |
|---------|--------------------------------|
| `err_arity.sz` | Llamada a función con menos argumentos de los declarados |
| `err_bang_nonbool.sz` | `!` aplicado a entero (no a booleano) |
| `err_bool_plus_int.sz` | `true + 1` — sumar booleano y entero |
| `err_bounds.sz` | Acceso a array fuera de rango |
| `err_call_undefined.sz` | Llamar a función que no existe |
| `err_div_zero.sz` | División entera por cero |
| `err_extra_iface_field.sz` | Interface instanciada con campo no declarado en ella |
| `err_for_scope_leak.sz` | Variable de `for` accedida fuera del bucle |
| `err_modulo_zero.sz` | Módulo por cero |
| `err_not_function.sz` | Intentar llamar a un valor que no es función |
| `err_overflow.sz` | Overflow de `i64` en multiplicación |
| `err_private.sz` | Llamar a método `private` desde fuera de la clase |
| `err_return_toplevel.sz` | `return` fuera de función |
| `err_return_type_mismatch.sz` | Función que retorna tipo distinto al declarado |
| `err_sort_mixed.sz` | `.sort()` en array con tipos mezclados incompatibles |
| `err_type_param.sz` | Pasar argumento de tipo incorrecto a función tipada |
| `err_typed_push.sz` | `.push()` de tipo incorrecto en array tipado |
| `err_undeclared_assign.sz` | Asignar a variable no declarada |
| `err_undeclared_class.sz` | `new Clase()` donde la clase no existe |
| `err_undeclared.sz` | Usar variable no declarada |
| `err_foreach_nonarray.sz` | `for (let x in 42)` — iterar sobre un entero (no iterable) |
| `err_foreach_dict.sz` | `for (let x in dict)` — iterar sobre un diccionario (no iterable) |

---

## Tests Unitarios (`unit_*`)

Los tests unitarios usan el framework de `tests/framework.sz`.
Cada caso llama a `test("nombre", () => { assert(...); })`.
Un fallo produce `[FAIL]` en stdout; el runner lo detecta.

---

### `unit_try_catch.sz` — Try/Catch básico (12 tests)

| Test | Qué verifica |
|------|--------------|
| catch receives thrown string | `throw "oops"` → `e == "oops"` en catch |
| catch receives thrown int | `throw 42` → `e == 42` en catch |
| code after throw in try does not run | Sentencias tras `throw` se saltan |
| finally runs on normal path | `finally` corre cuando no hay excepción |
| finally runs on throw path | `finally` corre tras `catch` |
| exception from function propagates to caller catch | `throw` dentro de `fn` se propaga al caller |
| nested try — inner catch, outer never sees it | Inner catch maneja: outer no dispara |
| nested try — inner re-throws, outer catches | Rethrow desde inner catch llega al outer |
| catch with return in function | `return` dentro de `catch` devuelve el valor correcto |
| assert throws on false | `assert(false, msg)` lanza `msg` |
| assert does NOT throw on true | `assert(true, msg)` no lanza |
| exception from class method propagates | `throw` dentro de método de clase se propaga |

---

### `unit_try_catch_edge.sz` — Try/Catch casos extremos (10 tests)

| Test | Qué verifica |
|------|--------------|
| return in try — return value preserved through finally | `return` en try body: el valor llega al caller aunque `finally` corra |
| throw in finally overrides try return | `finally` lanza: override sobre el `return` del try |
| throw in finally overrides normal try completion | `finally` lanza: override sobre completion normal del try |
| throw inside for loop propagates to outer catch | `throw` dentro de `for` → llega al catch que envuelve el for |
| throw inside while loop propagates to outer catch | `throw` dentro de `while` → llega al catch externo |
| try with only finally — local variable modified correctly | `try { } finally { }` sin `catch` es válido y funciona |
| finally-only try propagates throw | `try { throw } finally { }` → throw se propaga tras finally |
| catch body throws — propagates to outer catch | Lanzar desde dentro de `catch` → outer catch lo recibe |
| three-level nested try/rethrow chain | Tres niveles de catch anidados con rethrow encadenado |
| throw propagates through multiple function calls | `throw` a través de dos frames de función llega al catch |

---

### `unit_switch.sz` — Switch básico (8 tests)

| Test | Qué verifica |
|------|--------------|
| switch matches exact int | Case exacto con entero |
| switch matches exact string | Case exacto con string |
| switch default when no case matches | `default` se ejecuta si ningún case coincide |
| switch with multiple values per case | `case 1, 2:` — múltiples valores en un case |
| switch no match no default — skips cleanly | Sin match y sin default: no ejecuta nada, no falla |
| switch with expression as value | `switch (arr[1] / 10)` — expresión como discriminante |
| switch inside function returns correctly | `return` dentro de case de switch devuelve de la función |
| switch with bool | `case true:` / `case false:` |

---

### `unit_switch_edge.sz` — Switch casos extremos (9 tests)

| Test | Qué verifica |
|------|--------------|
| switch — no fall-through between cases | Solo el case que matchea corre; los siguientes no |
| switch with decimal values | `switch (1.5)` con `case 1.5:` |
| switch with null value | `switch (null)` con `case null:` |
| switch inside for loop — accumulates correctly | Switch dentro de for: cada iteración evalúa el switch |
| nested switch | Switch dentro de otro switch |
| throw inside switch case propagates | `throw` dentro de case llega al catch externo |
| switch inside for loop — break exits the loop | `break` dentro de case rompe el `for`, no el switch |
| switch default runs exactly once | Default corre exactamente 1 vez cuando no hay match |
| switch multiple values per case — middle value matches | Tercer valor de `case 7, 8, 9:` matchea correctamente |

---

### `unit_compound_assign.sz` — Asignación compuesta básica (11 tests)

| Test | Qué verifica |
|------|--------------|
| += on int | `10 += 5 → 15` |
| -= on int | `10 -= 3 → 7` |
| *= on int | `4 *= 3 → 12` |
| /= on int | `20 /= 4 → 5` |
| %= on int | `17 %= 5 → 2` |
| += on string | Concatena: `"hello" += " world"` |
| += on decimal | `1.5 += 0.5 → 2.0` |
| += accumulates in loop | Suma 1..10 con `sum += i` → 55 |
| += on array element | `arr[1] += 5` modifica el elemento correcto |
| *= on array element | `arr[0] *= 3` modifica el elemento correcto |
| += on instance field | `this.val += n` dentro de método de clase |

---

### `unit_compound_assign_edge.sz` — Asignación compuesta casos extremos (12 tests)

| Test | Qué verifica |
|------|--------------|
| -= on decimal | `5.0 -= 1.5 → 3.5` |
| /= on decimal | `10.0 /= 4.0 → 2.5` |
| *= on decimal | `3.0 *= 2.5 → 7.5` |
| -= on array element | `arr[1] -= 5` con verificación de elementos adyacentes |
| /= on array element | `arr[0] /= 4 → 25` |
| += on dict entry | `dict["alice"] += 5` modifica la entrada del diccionario |
| *= on dict entry | `dict["x"] *= 4` modifica la entrada del diccionario |
| -= on dict entry | `dict["n"] -= 37` modifica la entrada del diccionario |
| += on instance field directly | `c.val += 3` desde fuera de la clase |
| -= on instance field directly | `b.n -= 7` desde fuera de la clase |
| compound assign chain on same variable | `x += 5; x *= 2; x -= 6; x /= 4; x %= 4` → 2 |
| += accumulates across iterations with growing step | Acumulación con step creciente |

---

### `unit_operators.sz` — Operadores (15 tests)

| Test | Qué verifica |
|------|--------------|
| && short-circuits when left is false | `false && boom()` → boom jamás se llama |
| \|\| short-circuits when left is true | `true \|\| boom()` → boom jamás se llama |
| && evaluates right side when left is true | `true && true`, `true && false` |
| \|\| evaluates right side when left is false | `false \|\| true`, `false \|\| false` |
| ?? short-circuits when left is not null | `"valor" ?? boom()` → boom no se llama |
| ?? evaluates right when left is null | `null ?? "fallback"` → `"fallback"` |
| && evaluates right side — throw from right propagates | `true && fn_que_lanza()` → throw llega al catch |
| operator precedence: * before + | `2 + 3 * 4 = 14`, `10 - 2 * 3 = 4` |
| operator precedence: comparison after arithmetic | `2 + 3 > 4`, `10 / 2 == 5`, `3 * 3 >= 9` |
| chained boolean operations | `true && true && true`, combinaciones con `\|\|` |
| unary negation on int and decimal | `-5 = 0-5`, `-(-3) = 3`, `-1.5` |
| ! operator | `!false = true`, `!true = false`, `!!true = true` |
| string equality and inequality | `"a" == "a"`, `"a" != "b"` |
| integer comparison operators | `>`, `<`, `>=`, `<=`, `!=` sobre enteros |
| decimal comparison operators | `>`, `<`, `>=`, `==`, `!=` sobre decimales |

---

### `unit_closures_mutable.sz` — Closures con estado mutable (7 tests)

Cubre el patrón de closure que modifica su estado capturado entre llamadas: contadores, acumuladores, estado compartido.

| Test | Qué verifica |
|------|--------------|
| make_counter: cada llamada incrementa el estado | `make_counter()` retorna closure; llamadas sucesivas devuelven 1, 2, 3 |
| dos contadores independientes no comparten estado | Dos closures de `make_counter` tienen conteos separados |
| acumulador: suma valores entre llamadas | Closure que acumula suma entre llamadas: 10 → 15 → 40 → 30 |
| make_adder_from con estado inicial parametrizado | `make_adder_from(10)` inicia en 10 y acumula; independiente de `make_adder_from(0)` |
| closure captura variable de loop for y la mantiene | `captured = i` dentro del loop captura el valor correcto; fns[2]() == 4 |
| toggle: alterna estado bool entre llamadas | `make_toggle(false)` → true → false → true |
| closure acumula strings | Builder closure que concatena strings entre llamadas |

---

### `unit_closures_edge.sz` — Closures y HOF (9 tests)

| Test | Qué verifica |
|------|--------------|
| lambda captures value at creation — basic | `let f = x => x + base` usa `base` capturado |
| lambda returned from function — make_adder | `make_adder(5)` devuelve closure; `add5(3) = 8` |
| lambda returned from function — make_multiplier | `make_mult(2)` devuelve closure; composición de closures |
| higher-order composition: compose(f, g)(x) = f(g(x)) | `compose(inc, double)(5) = 11` |
| apply_twice: f(f(x)) | `apply_twice(double, 3) = 12`; `apply_twice(square, 2) = 16` |
| lambda as argument to user-defined HOF | `mi_map([1..5], x => x * 2)` con HOF propia |
| lambda with block body and multiple returns | Lambda multi-línea con varios `return` en ramas |
| closures used in map — each closure independent | Array de closures `[adder(1), adder(2), adder(3)]` independientes |
| lambda captures outer fn parameter — currying | `curry_add(3)` devuelve `inner` que suma 3 |

---

### `unit_forin_string.sz` — for-in sobre strings (10 tests)

Cubre la iteración carácter a carácter de strings con `for-in`.

| Test | Qué verifica |
|------|--------------|
| for-in string recolecta caracteres en orden | Itera `"hello"` y verifica orden y longitud |
| for-in string cuenta caracteres | `n++` por cada char de `"serez"` → 5 |
| for-in string vacío no itera | `""` → cero iteraciones |
| for-in string cuenta vocales | `"Hello World"` → 3 vocales (e, o, o) |
| for-in string reconstruye en mayúsculas | `"abc"` → `"ABC"` usando `toUpperCase()` por char |
| for-in string: break al encontrar carácter | Rompe al hallar `"-"` en `"serez-code"`, verifica posición |
| for-in string: continue salta espacios | Omite espacios en `"a b c"` → `"abc"` |
| for-in string en función: retorno anticipado | `primerDigito("abc3def") == 3` con `return` dentro del for-in |
| for-in string: resultado de split | Itera sobre `"uno,dos,tres".split(",")` |
| for-in string de un solo carácter | `"x"` produce exactamente un carácter |

---

### `unit_foreach_ternary_incr.sz` — ForEach, Ternario y ++/-- (22 tests)

| Test | Qué verifica |
|------|--------------|
| for-in sums array elements | `for (let n in nums)` suma todos los elementos de un `[int]` |
| for-in iterates in order | El orden de iteración es el orden del array |
| for-in over empty array does nothing | Un array vacío no ejecuta el cuerpo |
| for-in over string iterates characters | Itera sobre cada carácter de un `string` |
| for-in break exits early | `break` dentro del cuerpo detiene la iteración |
| for-in continue skips elements | `continue` salta el elemento actual |
| for-in nested loops | Dos `for-in` anidados con variables independientes |
| for-in with method on elements | Llamada a `.length()` sobre cada elemento string |
| ternary selects true branch | `true ? 1 : 2` produce `1` |
| ternary selects false branch | `false ? 1 : 2` produce `2` |
| ternary with expression condition | `n > 5 ? "big" : "small"` con variable |
| ternary is lazy — only evaluates chosen branch | La rama no elegida no se evalúa (`called == 0`) |
| ternary chained (right-associative) | `n == 1 ? "one" : n == 2 ? "two" : "other"` → `"two"` |
| ternary in expression | `a > b ? a : b` computa el máximo |
| ternary with null check | `val == null ? "was null" : "not null"` |
| postfix i++ increments by 1 | `i++` deja `i = i + 1` |
| postfix i-- decrements by 1 | `i--` deja `i = i - 1` |
| prefix ++i increments by 1 | `++i` deja `i = i + 1` |
| prefix --i decrements by 1 | `--i` deja `i = i - 1` |
| ++ inside while loop | `count++` usado como avance de bucle |
| -- countdown | `n--` en cuenta regresiva, `sum = 3+2+1 = 6` |
| ++ and -- together | `a++` y `b--` operan independientemente |

---

---

### `unit_foreach_edge.sz` — ForEach, Ternario y ++/-- casos extremos (18 tests)

| Test | Qué verifica |
|------|--------------|
| for-in return from function exits immediately | `return` dentro de `for-in` sale de la función completa |
| for-in throw caught by enclosing try-catch | `throw` dentro de `for-in` lo recibe el `catch` exterior |
| for-in over expression (split result) | `for (let w in "a,b,c".split(","))` itera resultado de método |
| for-in does not mutate the source array | El array fuente no se modifica durante la iteración |
| for-in closures capture each iteration independently | Closure creada en cada iteración captura su propio valor de `v` |
| for-in inside class method mutates this field | `for-in` dentro de método de clase puede mutar `this.total` |
| for-in ternary in body selects sign | Ternario dentro del cuerpo selecciona `"+"` o `"-"` por iteración |
| for-in with ++ counter | `count++` dentro de `for-in` cuenta correctamente las iteraciones |
| ternary as function return value | Ternario encadenado como `return`: `n>0 ? "positive" : n<0 ? "negative" : "zero"` |
| ternary result in array literal | `[a > b ? a : b, a < b ? a : b]` — ternario como elemento de array |
| ternary inside while condition | `while (i < (limit > 2 ? 5 : 3))` — ternario en condición de while |
| ternary in string interpolation | `"x is {x > 0 ? "positive" : "negative"}"` — ternario interpolado |
| ternary with ?? — ?? binds tighter | `val ?? "default" ? "yes" : "no"` = `(val ?? "default") ? "yes" : "no"` |
| ternary lazy — false branch with throw not evaluated | La rama falsa que contiene `throw` no se evalúa cuando la condición es true |
| ++ on global variable works | `g++; g++; ++g` desde scope global → `g == 3` |
| -- to zero and below | `n--` tres veces desde 2 → `-1` |
| ++ inside for-in body | `evens++` dentro de `for-in` con condición: cuenta sólo los pares |
| ++ and -- in nested while loops | `inner_total++` y `outer++`/`inner++` en while anidado → `outer==3`, `inner_total==9` |

---

### `unit_super_method.sz` — super.method() en métodos normales de clases hija (10 tests)

| Test | Qué verifica |
|------|--------------|
| super.method() no args dispatches to parent | `super.label()` llama a `Counter::label` literal "Counter", no al override de hijo |
| own overridden method not affected | El propio `label()` del hijo devuelve su override |
| super.method() returns value using this fields | `super.doubled()` usa `this.value` del hijo → correcto |
| super.method() with argument | `super.add(10)` con argumento — `3 + 10 = 13` |
| super.method() dispatches to parent override not own override | `super.describe()` llama `Counter::describe`, no `NamedCounter::describe` |
| super.method() result used in expression | `super.label() + " vs " + this.label()` en una expresión |
| 3-level: super.label() dispatches to NamedCounter::label | `TaggedCounter.super.label()` llama `NamedCounter::label` (no salta a `Counter`) |
| 3-level: own label() overrides all | El propio `label()` de `TaggedCounter` devuelve su override |
| 3-level: chained super through NamedCounter::parentLabel to Counter::label | `grandparentLabel()` encadena `super` → `NamedCounter::parentLabel` → `super.label()` → "Counter" |
| 3-level: this.value accessible via inherited super method | `parentDoubled()` a través de herencia usa `this.value` correcto |

### `unit_functions_adv.sz` — Funciones avanzadas (9 tests)

Cubre patrones funcionales no cubiertos en `unit_functions.sz`: múltiples defaults, recursión mutua, HOF avanzado.

| Test | Qué verifica |
|------|--------------|
| múltiples parámetros con valor por defecto | `formato(val, pre="[", suf="]")` con 0, 1 y 2 overrides |
| default override solo del primero | `suma(1)`, `suma(1,2)`, `suma(1,2,3)` con 2 defaults |
| recursión mutua: isEven / isOdd | `isEven`/`isOdd` se llaman mutuamente; correcto para n=0..7 |
| recursión de cola: suma 1..n con acumulador | `sumTo(n, acc=0)` tail-recursive; `sumTo(10) == 55` |
| función que retorna función basada en condición | `selector(true)` → doble, `selector(false)` → +100 |
| función almacenada en variable y reasignada | Variable `op` apunta a `doble` luego a `triple` |
| pipeline de funciones en array | Array de lambdas aplicadas en secuencia: `5 → 6 → 12 → 9` |
| función recursiva: pow con exponent negativo | `pow(2.0, 3) == 8.0`, `pow(2.0, -1) == 0.5` |
| función con parámetro any: dispatch por is | `describir(42)` → `"entero: 42"`, `describir(null)` → `"otro"` |

---

### `unit_class_patterns.sz` — Patrones de clase (8 tests)

Cubre patrones de diseño OOP: factory method, builder fluido, clase contador, campos array con HOF, método privado.

| Test | Qué verifica |
|------|--------------|
| factory method: método que retorna nueva instancia | `punto.trasladar(3,4)` retorna nuevo `Punto`; original no muta |
| class Counter con reset | `inc()`, `dec()`, `reset()` gestionan estado interno |
| clase con campo array y métodos sobre él | `Bolsa.agregar/quitar/tiene()` operan sobre `this.items` |
| herencia: clase hija extiende con método nuevo | `Circulo` hereda `id()` y agrega `area()` |
| método privado usado solo internamente | `Validator.clasificar()` usa método `private esPar()` internamente |
| builder pattern fluido | `QueryBuilder.from().where().limit().build()` encadenado |
| array de instancias con map y filter | `filter(p => p.precio > 20)` y `reduce` sobre array de `Producto` |
| clase Registry: almacena y recupera por nombre | `register("pi", 3.14)` luego `get("pi") == 3.14`; `get("nope") == null` |

---

### `unit_dict_advanced.sz` — Dicts avanzados (9 tests)

Cubre tipos de clave no-string, construcción dinámica, semántica de paso por valor, y patrones de agrupamiento.

| Test | Qué verifica |
|------|--------------|
| dict con clave int | `<int,string>` con claves 0, 1, 2; clave inexistente = null |
| dict `<int,int>`: operaciones numéricas | `cuadrados[3] == 9`, suma de valores |
| for-in sobre dict `<int,string>` | Itera claves enteras; `keys.includes(10)` |
| dict como parámetro: semántica por valor | Mutación en función NO persiste en el caller (pass-by-value) |
| dict construido dinámicamente con while loop | `d[i] = i*i` dentro de while; `d[3] == 9` tras el loop (B-60 fix) |
| dict como tabla de frecuencias | Cuenta ocurrencias con `freq[w] = (freq[w] ?? 0) + 1` |
| dict devuelto desde función | Función retorna `<string,any>` con distintos tipos de valor |
| dict de arrays: agrupar por categoría | `grupos["pares"]` y `grupos["impares"]` acumulan con `push` |
| dict: claves() y valores() en sintonía | `keys()` y `values()` tienen misma longitud; `reduce` sobre values |

---

### `unit_reverse_writeback.sz` — `.reverse()` muta en-lugar y retorna array (8 tests)

Cubre B-62: `.reverse()` debe mutar el array y devolver la referencia al mismo array (igual que `.sort()`).

| Test | Qué verifica |
|------|--------------|
| reverse mutates the array | `a.reverse(); a[0] == 5` — la mutación persiste |
| reverse returns the same array | `let b = a.reverse(); b[0] == 30` — el valor retornado es el array revertido |
| return value is the same array | `a` y `b` después de `b = a.reverse()` reflejan el mismo estado |
| reverse on empty array | `[].reverse()` sin error, length sigue siendo 0 |
| reverse on single element | `[42].reverse()` no cambia nada |
| double reverse restores order | `a.reverse(); a.reverse()` → estado original |
| reverse and then iterate | Suma de elementos es la misma antes y después del reverse |
| reverse works on string array | `["a","b","c"].reverse()` → `["c","b","a"]` |

---

### `unit_trim_aliases.sz` — `trimLeft` / `trimRight` como aliases (8 tests)

Cubre B-63: `trimLeft()` y `trimRight()` son aliases de `trimStart()` y `trimEnd()`.

| Test | Qué verifica |
|------|--------------|
| trimLeft removes leading whitespace | `"   hello".trimLeft() == "hello"` |
| trimRight removes trailing whitespace | `"hello   ".trimRight() == "hello"` |
| trimLeft and trimRight together | `s.trimLeft().trimRight()` equivale a `s.trim()` |
| trimLeft is identical to trimStart | Ambos producen el mismo resultado en el mismo string |
| trimRight is identical to trimEnd | Ambos producen el mismo resultado en el mismo string |
| trimLeft preserves trailing spaces | Solo elimina espacios iniciales, no los finales |
| trimRight preserves leading spaces | Solo elimina espacios finales, no los iniciales |
| all five trim variants consistent | `trim`, `trimStart`, `trimEnd`, `trimLeft`, `trimRight` en sintonía |

---

### `unit_comprehensive_new.sz` — Cobertura profunda de características nuevas (33 tests)

Tests unitarios exhaustivos de todas las características añadidas: `const`, `enum`, labeled loops, clases `abstract`/`sealed`, getters/setters, métodos estáticos, parámetros por defecto, optional chaining, `do-while`, bitwise/power, spread/rest, `Math`, `JSON`, `Set`, y operador `is`.

| Test | Área |
|------|------|
| const prevents reassignment | `const` — inmutabilidad forzada en runtime |
| const in different type contexts | `const` para int, string, bool, decimal |
| enum variant access | `Color.Red == Color.Red`, distintos variantes no son iguales |
| enum in conditional | `if (prio == Priority.High)` |
| labeled break exits outer loop | `outer: for ... break outer` |
| labeled continue skips outer iteration | `outer: for ... continue outer` |
| abstract class cannot be instantiated | `new AbstractBase()` lanza error no-catchable |
| sealed class cannot be inherited | Herencia de sealed = error de tipo |
| getter returns computed value | `get area()` calcula en el momento |
| setter validates and stores | `set value(v)` valida entrada |
| static method called on class | `MathHelper.add(3, 4)` sin instancia |
| default parameter single | `greet("Bob")` usa `"Hello"` como default |
| default parameter override | `greet("Bob", "Hi")` usa el override |
| optional chaining short-circuits | `null?.method` retorna null sin error |
| optional chaining with ?? | `obj?.field ?? "default"` |
| do-while executes at least once | El cuerpo corre aunque la condición sea false inicial |
| do-while with break | `break` dentro de do-while |
| bitwise AND / OR / XOR | `5 & 3 == 1`, `5 \| 3 == 7`, `5 ^ 3 == 6` |
| bitwise NOT | `~0 == -1`, `~7 == -8` |
| shift operators | `1 << 3 == 8`, `8 >> 2 == 2` |
| power operator | `2 ** 10 == 1024`, `3 ** 3 == 27` |
| spread in array literal | `[...a, ...b]` concatena |
| rest parameters | `fn sum(...nums)` acumula argumentos variables |
| Math namespace | `Math.abs`, `Math.floor`, `Math.ceil`, `Math.sqrt`, `Math.PI` |
| Math.min / Math.max variadic | `Math.min(3, 1, 4, 1, 5)`, `Math.max(...)` |
| JSON.stringify primitives | int, string, bool, null a JSON |
| JSON.stringify array | `[1,2,3]` → `"[1,2,3]"` |
| JSON.parse roundtrip | stringify → parse → mismo valor |
| Set deduplication | `new Set(["a","b","a"])` → size 2 |
| Set operations | `add`, `has`, `delete`, `clear`, `toArray` |
| is type check on primitives | `42 is int`, `"x" is string`, `true is bool` |
| is type check on instances | `obj is ClassName` |
| is type check in catch | `e is NetworkError` dispatch en catch |

---

## Resumen de cobertura

| Área | E2E | Unit | Error | Total |
|------|-----|------|-------|-------|
| Tipos primitivos y aritmética | 01_basic, 01_arithmetic, 02_arithmetic, 22_math_edge | unit_operators (parcial) | err_overflow, err_bool_plus_int | ~40 casos |
| Variables y scoping | 01_variables, 02_variables, 02_variables_scope | — | err_undeclared, err_undeclared_assign, err_for_scope_leak | ~15 casos |
| Control de flujo | 03_control_flow, 04_control_flow | — | — | ~12 casos |
| Funciones y recursión | 04_functions, 05_functions, 17_function_syntax | unit_functions_adv (9) | err_arity, err_return_toplevel, err_return_type_mismatch, err_type_param | ~30 casos |
| Strings | 03_strings, 06_strings, 21_string_interp_complex, 27_escape_sequences | — | — | ~25 casos |
| Arrays | 05_arrays, 06_arrays, 23_boundary_cases | unit_compound_assign (parcial) | err_bounds, err_typed_push, err_sort_mixed | ~30 casos |
| Diccionarios | 07_dicts | unit_dict_advanced (9) + unit_compound_assign_edge (parcial) | — | ~22 casos |
| Clases e herencia | 08_classes, 30_class_regression | unit_class_patterns (8) + unit_super_method (10) | err_private, err_undeclared_class | ~40 casos |
| Interfaces | 09_interfaces | — | err_extra_iface_field | ~8 casos |
| Lambdas y closures | 10_lambdas, 26_complex_scenarios | unit_closures_edge (9) + unit_closures_mutable (7) | — | ~35 casos |
| Nullables | 11_nullables | — | — | ~8 casos |
| Matemáticas | 12_math, 22_math_edge | — | err_div_zero, err_modulo_zero | ~12 casos |
| Try/Catch/Throw/Finally | 33_try_catch | unit_try_catch (12) + unit_try_catch_edge (10) | — | 32 casos |
| Switch | 32_switch | unit_switch (8) + unit_switch_edge (9) | — | 23 casos |
| Compound assign | 31_compound_assign | unit_compound_assign (11) + unit_compound_assign_edge (12) | — | 34 casos |
| Operadores | 14_arch_features, 18_error_cases | unit_operators (15) | err_bang_nonbool | 20 casos |
| Regresiones | 29_bug_regression | — | — | ~25 casos |
| Casos extremos | 13_edge_cases, 15_arch_stress, 20_more_edge_cases, 23_boundary_cases, 28_final_checks | — | — | ~40 casos |
| ForEach / Ternario / ++-- | — | unit_foreach_ternary_incr (22) + unit_foreach_edge (18) + unit_forin_string (10) | err_foreach_nonarray, err_foreach_dict | 50 casos |
| const / enum | unit_comprehensive_new (parcial) | — | — | ~8 casos |
| abstract / sealed / static / default params | unit_comprehensive_new (parcial) | — | — | ~8 casos |
| optional chaining / do-while | unit_comprehensive_new (parcial) | — | — | ~4 casos |
| Bitwise / power | unit_comprehensive_new (parcial) + unit_bitwise_edge | err_negative_shift, err_excessive_shift | — | ~12 casos |
| Spread / rest | unit_comprehensive_new (parcial) | — | — | ~4 casos |
| Math namespace | 12_math, 22_math_edge | unit_comprehensive_new (parcial) | — | ~10 casos |
| JSON namespace | unit_comprehensive_new (parcial) + 38_real_programs (prog 5) | — | — | ~8 casos |
| Set | unit_comprehensive_new (parcial) + 38_real_programs (prog 2) | — | — | ~6 casos |
| is type check | unit_is_type_advanced + unit_comprehensive_new (parcial) | — | — | ~10 casos |
| Excepciones avanzadas | 37_exceptions_e2e | unit_exceptions_advanced + unit_comprehensive_new (parcial) | sec_runtime_not_catchable | ~20 casos |
| Programas reales integrados | 38_real_programs (8 programas) | — | — | ~80 casos |
| `.reverse()` write-back | — | unit_reverse_writeback (8) | — | 8 casos |
| trimLeft / trimRight aliases | — | unit_trim_aliases (8) | — | 8 casos |
