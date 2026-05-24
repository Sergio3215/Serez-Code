# Serez-Code Formatter

Syntax highlighting and formatter for **Serez-Code** (`.sz`) files — a hand-crafted interpreted language written in Rust.

## Features

- **Syntax highlighting** for keywords, types, operators, strings, numbers and comments
- **Auto-formatter** on save (configurable via `editor.formatOnSave`)
- Keywords: `fn`, `let`, `const`, `class`, `interface`, `enum`, `if`, `while`, `for`, `switch`, `try`, `throw`, ...
- Types: `int`, `decimal`, `string`, `bool`, `any`, `void`
- String interpolation: `"Hello, {name}!"`
- Numbers: integers, decimals, hex (`0xFF`), binary (`0b1010`), separators (`1_000_000`)
- Operators: arithmetic, bitwise, logical, `??`, `?.`, `**`, `is`
- Comments: `//` and `/* */`
- Bracket matching and auto-close

## Usage

Files with the `.sz` extension are automatically detected and formatted on save.

## Installation

### Open VSX / Antigravity IDE

Search for **Serez-Code Formatter** in the Extensions marketplace.

Or install manually from a `.vsix` file:

```bash
antigravity-ide.cmd --install-extension serez-code-formatter-1.1.0.vsix
```

### VS Code

```bash
code --install-extension serez-code-formatter-1.1.0.vsix
```

## Language

[Serez-Code on GitHub](https://github.com/Sergio3215/serez-code)
