# Serez-Code Formatter

Syntax highlighting, formatter, **language server client** and a dedicated **color theme** for **Serez-Code** — a hand-crafted interpreted language written in Rust. Supports the three file kinds of the ecosystem: `.sz` (code), `.szx` (JSX authoring for serez-ui) and `.szs` (CSS-with-logic styles).

## Features

- **Language server (desde 1.7.0)** — si el binario `sz-lsp` está disponible
  (en el `PATH` junto a `sz`, o vía el setting `serez.lsp.path`), los `.sz`
  ganan: **diagnósticos en vivo** (errores del parser + avisos del type
  checker mientras escribes), **autocompletado** (keywords, namespaces
  nativos con sus métodos — `File.` lista `read`/`write`/… —, funciones y
  variables del archivo), **hover** con firmas, **ir a definición** (F12,
  incluidas líneas `import`) y **outline** de símbolos. Se construye con
  `cargo build --release --bin sz-lsp` en el repo del core. Desactivable con
  `serez.lsp.enabled: false`; sin el binario, la extensión sigue funcionando
  como siempre (resaltado + formatter).
- **Syntax highlighting**
  - `.sz` — keywords, types, **built-in namespaces** (`Math`, `JSON`, `File`, `Terminal`, `OS`, `Env`, `Time`, `DateTime`, `System`, `Random`, `Tensor`, `Autodiff`, `Gui`, `Crypto`, `Socket`, `Set`, `Binary`, `GPU`, `Memory`), operators, strings, numbers, comments
  - `.szx` — everything in `.sz` **plus JSX** (tags, components, attributes, `{…}` embedded expressions)
  - `.szs` — CSS styling **plus** Serez extras: `:import` block and reactive conditions `selector (count == 0) { … }`
- **Serez Dark color theme** — a built-in theme matching the Serez palette (deep violet + cyan on near-black). Pick it from *Preferences → Color Theme → Serez Dark*.
- **Auto-formatter** on save for `.sz` (brace-based indenter; configurable via `editor.formatOnSave`)
- Keywords: `import`, `export`, `use` / `permissions`, `fn`, `let`, `const`, `class`, `interface`, `enum`, `if`, `while`, `for`, `switch`, `match`, `try`, `throw`, `is`, `unsafe`, `yield`, …
- Types: `int`, `decimal`, `dec` (exact decimal), `string`, `bool`, `any`, `void`
- String interpolation: `"Hola, {name}!"` · raw strings: `r"C:\path\no\escapes"`
- Numbers: integers, decimals, `dec` literals (`199.99m`), scientific notation (`1e-7`), hex (`0xFF`), binary (`0b1010`), separators (`1_000_000`)
- Operators: arithmetic, bitwise, logical, `??`, `?.`, `**`, `=>`, `...`
- Comments: `//` and `/* */`
- Bracket matching and auto-close

## Usage

Files with `.sz`, `.szx` or `.szs` are detected automatically. `.sz` files are formatted on save.

## Installation

### Open VSX / Antigravity IDE

Search for **Serez-Code Formatter** in the Extensions marketplace, or install a `.vsix` manually:

```bash
antigravity-ide.cmd --install-extension serez-code-formatter-1.6.0.vsix
```

### VS Code

```bash
code --install-extension serez-code-formatter-1.6.0.vsix
```

## Language

[Serez-Code on GitHub](https://github.com/Sergio3215/serez-code)
