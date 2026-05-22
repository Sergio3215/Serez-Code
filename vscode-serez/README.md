# Serez-Code

Syntax highlighting for **Serez-Code** (`.sz`) files — a hand-crafted interpreted language written in Rust.

## Features

- Keywords: `fn`, `let`, `const`, `class`, `interface`, `enum`, `if`, `while`, `for`, `switch`, `try`, `throw`, ...
- Types: `int`, `decimal`, `string`, `bool`, `any`, `void`
- String interpolation: `"Hello, {name}!"`
- Numbers: integers, decimals, hex (`0xFF`), binary (`0b1010`), separators (`1_000_000`)
- Operators: arithmetic, bitwise, logical, `??`, `?.`, `**`, `is`
- Comments: `//` and `/* */`
- Bracket matching and auto-close

## Usage

Files with the `.sz` extension are automatically detected.

## Installation

### VS Code

Install via the Extensions marketplace, or from a `.vsix` file:

```bash
code --install-extension serez-code-0.1.0.vsix
```

### Antigravity IDE (fork of VS Code)

Use the `antigravity-ide.cmd` launcher instead of `code`:

```bash
antigravity-ide.cmd --install-extension serez-code-0.1.0.vsix
```

## Language

[Serez-Code on GitHub](https://github.com/Sergio3215/serez-code)
