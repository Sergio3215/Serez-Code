# Serez-Code 🐒

Intérprete del lenguaje de programación **Serez-Code** escrito en **Rust**.

---

## Requisitos

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)

## Instalación y ejecución

```bash
git clone https://github.com/Sergio3215/serez-code.git
cd serez-code
cargo run
```

El REPL se inicia automáticamente:

```
Hello Sergio! This is the Serez-Code programming language!
Feel free to type in commands
>>
```

---

## Arquitectura

El intérprete sigue el pipeline clásico de un lenguaje interpretado:

```
Código fuente (texto)
       │
       ▼
   [ Lexer ]        → convierte caracteres en Tokens
       │
       ▼
   [ Parser ]       → convierte Tokens en un AST (Pratt Parser)
       │
       ▼
   [ Evaluator ]    → recorre el AST y produce valores (Objects)
       │
       ▼
   [ REPL ]         → imprime el resultado y espera la próxima línea
```

### Archivos

| Archivo | Responsabilidad |
|---|---|
| `src/token.rs` | Define los tipos de tokens (`TokenType`) y la estructura `Token` |
| `src/lexer.rs` | Convierte el texto de entrada en una secuencia de `Token` |
| `src/ast.rs` | Define los nodos del AST: `Program`, `Statement`, `Expression` |
| `src/parser.rs` | Construye el AST a partir de los tokens usando un **Pratt Parser** |
| `src/evaluator.rs` | Evalúa el AST y gestiona el entorno de variables (`HashMap`) |
| `src/repl.rs` | Bucle interactivo: lee línea, parsea, evalúa e imprime |
| `src/main.rs` | Punto de entrada — saluda al usuario e inicia el REPL |

---

## Tokens implementados

### Literales e identificadores
| Token | Ejemplo |
|---|---|
| `Ident` | `mi_variable`, `x`, `resultado` |
| `Int` | `42`, `0`, `1000` |
| `String` | `"hola mundo"` |
| `True` / `False` | `true`, `false` |

### Operadores
| Token | Símbolo | Descripción |
|---|---|---|
| `Assign` | `=` | Asignación / reasignación |
| `Plus` | `+` | Suma / concatenación de strings |
| `Minus` | `-` | Resta / negación unaria |
| `Asterisk` | `*` | Multiplicación / repetición de string |
| `Slash` | `/` | División entera |
| `Bang` | `!` | Negación booleana |
| `Eq` | `==` | Igualdad |
| `NotEq` | `!=` | Desigualdad |
| `Lt` | `<` | Menor que |
| `Gt` | `>` | Mayor que |

### Delimitadores
`(` `)` `{` `}` `[` `]` `,` `;`

### Palabras clave
`let` `fn` `if` `else` `return` `true` `false`

> **Nota:** `fn`, `if`, `else` y `return` son reconocidos por el Lexer pero **aún no evaluados** por el intérprete.

---

## AST — Nodos implementados

### Statements (sentencias)

```
Statement
├── Let(LetStatement)        →  let nombre = expresión;
├── Assign(AssignStatement)  →  nombre = expresión;
└── Expression(Expression)   →  cualquier expresión suelta
```

### Expressions (expresiones)

```
Expression
├── Identifier(String)                          →  x
├── Integer(i64)                                →  42
├── String(String)                              →  "hola"
├── Boolean(bool)                               →  true / false
├── ArrayLiteral(Vec<Expression>)               →  [1, 2, 3]
├── Prefix(operador, Expression)                →  -5  !true
└── Infix(Expression, operador, Expression)     →  a + b  x == y
```

---

## Evaluador

### Tipos de objeto (`Object`)

| Tipo | Descripción | Ejemplo |
|---|---|---|
| `Integer(i64)` | Número entero con signo de 64 bits | `Integer(42)` |
| `String(String)` | Cadena de texto | `String("hola")` |
| `Boolean(bool)` | Valor booleano | `Boolean(true)` |
| `Array(Vec<Object>)` | Arreglo de objetos | `Array([Integer(1), Integer(2)])` |
| `Null` | Ausencia de valor (resultado de `let`) | `Null` |

### Entorno de variables

Las variables se almacenan en un `HashMap<String, Object>` que **persiste durante toda la sesión del REPL**.

---

## Referencia del lenguaje

### Declaración de variables

```
let x = 10;
let nombre = "Sergio";
let activo = true;
let numeros = [1, 2, 3];
```

### Reasignación

```
let x = 5;
x = 99       // → Integer(99)
x            // → Integer(99)
```

> La reasignación solo funciona sobre variables **ya declaradas** con `let`.

### Aritmética entera

```
1 + 2        // → Integer(3)
10 - 3       // → Integer(7)
4 * 5        // → Integer(20)
10 / 3       // → Integer(3)   (división entera)
10 % 3       // → Integer(1)   (módulo)
```

### Operadores de comparación

```
5 > 3        // → Boolean(true)
5 < 3        // → Boolean(false)
5 == 5       // → Boolean(true)
5 != 3       // → Boolean(true)
```

### Operadores de prefijo

```
-5           // → Integer(-5)
!true        // → Boolean(false)
!false       // → Boolean(true)
```

### Operaciones con strings

```
"hola" + " mundo"    // → String("hola mundo")
"ha" * 3             // → String("hahaha")    (String × Integer)
"abc" == "abc"       // → Boolean(true)
"abc" != "xyz"       // → Boolean(true)
```

### Expresiones agrupadas

```
(2 + 3) * 4          // → Integer(20)
```

### Arrays

```
let arr = [1, 2, 3];
[10, 20, 30]         // → Array([Integer(10), Integer(20), Integer(30)])
```

---

## Precedencia de operadores

De menor a mayor precedencia:

| Nivel | Operadores |
|---|---|
| 1 — Lowest | (base) |
| 2 — Equals | `==` `!=` |
| 3 — LessGreater | `<` `>` |
| 4 — Sum | `+` `-` |
| 5 — Product | `*` `/` |
| 6 — Prefix | `-x` `!x` |
| 7 — Call | `fn(x)` |
| 8 — Index | `arr[i]` |

---

## Roadmap — Por implementar

- [ ] `if / else` — condicionales
- [ ] `return` — sentencia de retorno
- [ ] `fn` — definición y llamada de funciones
- [ ] Indexado de arrays: `arr[0]`
- [ ] `while` / loops
- [ ] `null` como valor explícito
- [ ] Manejo de errores estructurado (en vez de `println!`)
- [ ] Closures / funciones de orden superior

---

## Ejemplo de sesión

```
Hello Sergio! This is the Serez-Code programming language!
Feel free to type in commands
>> let a = 5
Null
>> let b = 10
Null
>> a + b
Integer(15)
>> a * b
Integer(50)
>> a > b
Boolean(false)
>> a = 99
Integer(99)
>> a
Integer(99)
>> let saludo = "hola"
Null
>> saludo + " mundo"
String("hola mundo")
>> "ja" * 4
String("jajajaja")
>> [1, 2, 3]
Array([Integer(1), Integer(2), Integer(3)])
```

---

## Contribuir

¡Las contribuciones son bienvenidas! Serez-Code es un proyecto open source y cualquier aporte es apreciado.

### ¿Cómo contribuir?

1. **Fork** el repositorio en GitHub
2. Crea una rama para tu feature o fix:
   ```bash
   git checkout -b feature/mi-nueva-feature
   ```
3. Haz tus cambios y **commitéalos** con un mensaje claro:
   ```bash
   git commit -m "feat: agrega soporte para if/else"
   ```
4. Sube tu rama y abre un **Pull Request**:
   ```bash
   git push origin feature/mi-nueva-feature
   ```

### Guías para contribuir

- Seguí el estilo del código existente (Rust idiomático)
- Documentá las funciones y módulos nuevos con comentarios
- Si agregás una feature al lenguaje, actualizá este README con ejemplos
- Preferí commits pequeños y atómicos con mensajes descriptivos

### Ideas para contribuir

Revisá el [Roadmap](#roadmap--por-implementar) — cualquier ítem pendiente es una oportunidad para aportar.

---

## Licencia

Este proyecto está licenciado bajo la **MIT License**.

Esto significa que podés:
- ✅ Usar el código libremente (incluso en proyectos comerciales)
- ✅ Modificarlo y distribuirlo
- ✅ Incluirlo en tus propios proyectos

Siempre que mantengas el aviso de copyright original.

Consultá el archivo [`LICENSE`](./LICENSE) para el texto completo.

---

<p align="center">
  Hecho con ❤️ y Rust por <a href="https://www.serez.dev">Serez Dev</a>
</p>
