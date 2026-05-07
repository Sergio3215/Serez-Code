# Serez-Code 🐒

Intérprete del lenguaje de programación **Serez-Code** escrito en **Rust**.

---

## Requisitos

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)

## Instalación y ejecución

```bash
git clone https://github.com/Sergio3215/serez-code.git
cd serez-code
cargo install --path .
```

### Ejecutar un script (`.sz`)

El intérprete se instalará globalmente en tu sistema como el ejecutable `sz`. Puedes pasarle un archivo con código Serez-Code:

```bash
sz mi_script.sz
```

### Modo Interactivo (REPL)

Si ejecutas el binario sin argumentos, se iniciará el REPL:

```
Hello Sergio! This is the Serez-Code programming language!
Feel free to type in commands
>>
```

---

## Arquitectura

El intérprete sigue el pipeline clásico de un lenguaje interpretado con un sistema de memoria especializado (Flash Scopes):

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
   [ Evaluator ]    → recorre el AST y produce valores (Arena Memory)
       │
       ▼
   [ Output ]       → imprime el resultado o maneja el REPL
```

### Archivos

| Archivo              | Responsabilidad                                                         |
| -------------------- | ----------------------------------------------------------------------- |
| `src/token.rs`     | Define los tipos de tokens (`TokenType`) y la estructura `Token`    |
| `src/lexer.rs`     | Convierte el texto de entrada en una secuencia de `Token`             |
| `src/ast.rs`       | Define los nodos del AST:`Program`, `Statement`, `Expression`     |
| `src/parser.rs`    | Construye el AST a partir de los tokens usando un **Pratt Parser** |
| `src/evaluator.rs` | Evalúa el AST gestionando Scopes y*Flash Scopes* (`EvalResult`)    |
| `src/region.rs`    | Sistema de **Region-Based Memory** (Arena) para evitar GC        |
| `src/scope.rs`     | Manejo del `ScopeStack` y *watermarks* $O(1)$                     |
| `src/repl.rs`      | Bucle interactivo: lee línea, parsea, evalúa e imprime                |
| `src/main.rs`      | Punto de entrada (CLI y ejecución de archivos `.sz`)                 |

---

## Tokens implementados

### Literales e identificadores

| Token                | Ejemplo                                |
| -------------------- | -------------------------------------- |
| `Ident`            | `mi_variable`, `n1`, `resultado` |
| `Int`              | `42`, `0`, `1000`                |
| `String`           | `"hola mundo"`                       |
| `True` / `False` | `true`, `false`                    |

### Operadores

| Token        | Símbolo | Descripción                            |
| ------------ | -------- | --------------------------------------- |
| `Assign`   | `=`    | Asignación / reasignación sub-global  |
| `Plus`     | `+`    | Suma / concatenación de strings        |
| `Minus`    | `-`    | Resta / negación unaria                |
| `Asterisk` | `*`    | Multiplicación / repetición de string |
| `Slash`    | `/`    | División entera                        |
| `Bang`     | `!`    | Negación booleana                      |
| `Eq`       | `==`   | Igualdad                                |
| `NotEq`    | `!=`   | Desigualdad                             |
| `Lt`       | `<`    | Menor que                               |
| `Gt`       | `>`    | Mayor que                               |
| `Arrow`    | `=>`   | Funciones flecha                        |

### Delimitadores

`(` `)` `{` `}` `[` `]` `,` `;`

### Palabras clave

`let` `fn` `if` `else` `return` `true` `false`

### Tipos de Datos

`void` `int` `string` `bool`

---

## AST — Nodos implementados

### Statements (sentencias)

```
Statement
├── Let(LetStatement)                         →  let nombre = expresión;
├── Assign(AssignStatement)                   →  nombre = expresión;
├── Return(ReturnStatement)                   →  return expresión;
├── Block(BlockStatement)                     →  { ... }
├── FunctionDeclaration(FunctionDeclaration)  →  fn int sumar() { ... }
└── Expression(Expression)                    →  cualquier expresión suelta
```

### Expressions (expresiones)

```
Expression
├── Identifier(String)                          →  x
├── Integer(i64)                                →  42
├── String(String)                              →  "hola"
├── Boolean(bool)                               →  true / false
├── ArrayLiteral(Vec<Expression>)               →  [1, 2, 3]
├── FunctionLiteral(FunctionLiteral)            →  int () => { ... }
├── Call(CallExpression)                        →  sumar(1, 2)
├── Prefix(operador, Expression)                →  -5  !true
└── Infix(Expression, operador, Expression)     →  a + b  x == y
```

---

## Evaluador y Memoria (Flash Scopes)

### Gestión de Memoria (Region-Based Memory)

Serez-Code **no utiliza un Garbage Collector tradicional**. En su lugar, usa un sistema de *Arenas* y *Watermarks* que garantizan velocidad y control determinista.

- **Global Arena:** Persiste durante toda la ejecución.
- **Flash Scopes:** Cada vez que se abre un bloque `{ ... }`, se toma un *watermark* de la memoria. Al cerrarse, **toda la memoria creada localmente se destruye de forma atómica e instantánea** $O(1)$.

### Valores de Retorno (Return Promotion)

Para evitar que el valor de un `return` sea destruido por el *Flash Scope*, Serez-Code aplica la técnica de **Return Unwinding & Promotion**, clonando dinámicamente el resultado retornado y promoviéndolo al scope superior antes de aniquilar la arena local.

### Reasignación "Sub-Global"

Se puede modificar una variable que esté en un scope superior haciendo `variable = nuevo_valor` sin usar `let`. El evaluador escalará los ámbitos hasta encontrarla y la modificará de forma in-place, incluso si el Flash Scope local muere.

---

## Referencia del lenguaje

### Declaración de variables y Reasignación

```serez
let x = 10;
let nombre = "Sergio";
let activo = true;

// Reasignación in-place (Sub-global)
x = 99;
```

### Funciones y Parámetros Híbridos

Serez-Code admite parámetros fuertemente tipados combinados con parámetros dinámicos (`Any`):

```serez
// 'n1' es Any, 'n2' requiere un Integer estricto.
fn int sumar(n1, int n2) {
    return n1 + n2;
}

sumar(5, 10);
```

#### Funciones Flecha (Arrow Functions)

```serez
let restar = int (a, b) => { 
    return a - b; 
};
```

#### Funciones Anónimas

```serez
let multiplicar = fn int(a, b) { return a * b; };
```

---

## Precedencia de operadores

De menor a mayor precedencia:

| Nivel            | Operadores    |
| ---------------- | ------------- |
| 1 — Lowest      | (base)        |
| 2 — Equals      | `==` `!=` |
| 3 — LessGreater | `<` `>`   |
| 4 — Sum         | `+` `-`   |
| 5 — Product     | `*` `/`   |
| 6 — Prefix      | `-x` `!x` |
| 7 — Call        | `fn(x)`     |
| 8 — Index       | `arr[i]`    |

---

## Roadmap — Por implementar

- [X] `return` — sentencia de retorno con Flash Scopes
- [X] `fn` — definición y llamada de funciones (Tradicionales y Arrow)
- [X] Parámetros híbridos y validación de tipos
- [X] Archivos CLI `.sz`
- [ ] `if / else` — condicionales
- [ ] Indexado de arrays: `arr[0]`
- [ ] `while` / loops
- [ ] Manejo de errores estructurado (en vez de `println!`)

---

## Ejemplo de sesión

```serez
fn int fibonacci(int x) {
    // Proximamente con condicionales if/else
    return x + 1;
}

let res = fibonacci(10);
res; // -> Integer(11)
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

Consultá el archivo [`LICENSE`](./LICENSE) para el texto completo.

---

<p align="center">
  Hecho con ❤️ y Rust por <a href="https://www.serez.dev">Serez Dev</a>
</p>
