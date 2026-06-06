# Serez-Code Formatter

Syntax highlighting and formatter for **Serez-Code** — a hand-crafted interpreted language written in Rust. Supports the three file kinds of the ecosystem: `.sz` (code), `.szx` (JSX authoring for serez-ui) and `.szs` (CSS-with-logic styles).

## Features

- **Syntax highlighting**
  - `.sz` — keywords, types, **built-in namespaces** (`Math`, `JSON`, `File`, `Terminal`, `OS`, `Env`, `Time`, `System`, `Random`, `Tensor`, `Autodiff`, `Gui`, `Crypto`, `Socket`, `Set`), operators, strings, numbers, comments
  - `.szx` — everything in `.sz` **plus JSX** (tags, components, attributes, `{…}` embedded expressions)
  - `.szs` — CSS styling **plus** Serez extras: `:import` block and reactive conditions `selector (count == 0) { … }`
- **Auto-formatter** on save for `.sz` (brace-based indenter; configurable via `editor.formatOnSave`)
- Keywords: `import`, `export`, `fn`, `let`, `const`, `class`, `interface`, `enum`, `if`, `while`, `for`, `switch`, `try`, `throw`, `is`, …
- Types: `int`, `decimal`, `string`, `bool`, `any`, `void`
- String interpolation: `"Hola, {name}!"`
- Numbers: integers, decimals, hex (`0xFF`), binary (`0b1010`), separators (`1_000_000`)
- Operators: arithmetic, bitwise, logical, `??`, `?.`, `**`, `=>`, `...`
- Comments: `//` and `/* */`
- Bracket matching and auto-close

## Usage

Files with `.sz`, `.szx` or `.szs` are detected automatically. `.sz` files are formatted on save.

## Installation

### Open VSX / Antigravity IDE

Search for **Serez-Code Formatter** in the Extensions marketplace, or install a `.vsix` manually:

```bash
antigravity-ide.cmd --install-extension serez-code-formatter-1.5.0.vsix
```

### VS Code

```bash
code --install-extension serez-code-formatter-1.5.0.vsix
```

## Language

[Serez-Code on GitHub](https://github.com/Sergio3215/serez-code)
