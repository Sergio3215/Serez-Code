# Contribuir a Serez-Code

Gracias por querer contribuir. Este documento explica cómo reportar issues, proponer cambios y enviar pull requests de forma que el proceso sea claro para todos.

---

## Reportar un issue

Antes de abrir un issue, buscá si ya existe uno similar abierto o cerrado.

### Título del issue

El título debe ser claro y conciso. Seguí este formato según el tipo:

| Tipo | Formato | Ejemplo |
|---|---|---|
| Bug | `[BUG] Descripción corta del problema` | `[BUG] flat(n) solo aplana un nivel` |
| Feature request | `[FEATURE] Lo que querés agregar` | `[FEATURE] Operador de módulo negativo` |
| Documentación | `[DOCS] Qué falta o está mal` | `[DOCS] Ejemplo de Set.union incorrecto` |
| Pregunta | `[QUESTION] Tu pregunta` | `[QUESTION] Cómo funciona el Flash Scope` |

### Cuerpo del issue

**Para bugs**, incluí:
- Qué hiciste (código `.sz` mínimo que reproduce el problema)
- Qué esperabas que pasara
- Qué pasó en realidad (mensaje de error o output incorrecto)
- Versión de `sz` (`sz --version`)
- Sistema operativo

**Para features**, incluí:
- Qué querés agregar y para qué sirve
- Ejemplo de cómo se vería la sintaxis o el comportamiento
- Si ya tenés una idea de cómo implementarlo, mencionalo

---

## Proponer un cambio (antes de escribir código)

Para cambios grandes — nueva sintaxis, cambios al evaluador, modificaciones al modelo de memoria — abrí un issue primero y describí qué querés hacer. Esto evita que trabajes en algo que no va a ser mergeado.

Para cambios pequeños (fix de typo en docs, corrección de un bug simple), podés ir directo al PR.

---

## Flujo de trabajo

### 1. Fork y clone

```bash
git clone https://github.com/<tu-usuario>/serez-code
cd serez-code
cargo build
```

### 2. Nombrar la rama

La rama debe describir exactamente qué implementa. Usá el número del issue si existe:

```
# Con issue asociado:
feature/123-do-while-loop
fix/87-flat-depth-parameter
docs/45-update-set-examples

# Sin issue (contribución directa):
feature/string-repeat-method
fix/parser-error-recovery-semicolon
docs/add-closures-example
```

**Prefijos válidos:**

| Prefijo | Cuándo usarlo |
|---|---|
| `feature/` | Nueva funcionalidad del lenguaje o tooling |
| `fix/` | Corrección de bug |
| `docs/` | Solo documentación |
| `refactor/` | Cambio interno sin cambio de comportamiento |
| `test/` | Agregar o corregir tests |
| `ci/` | Cambios al pipeline de CI/CD |

Evitá nombres genéricos como `mi-rama`, `cambios`, `fix`, `patch`.

### 3. Hacer los cambios

- Un commit por cambio lógico
- El mensaje del commit debe explicar el **por qué**, no solo el qué
- Si el cambio cierra un issue, incluí `Closes #123` en el cuerpo del commit

```bash
git commit -m "fix: flat(n) ahora aplana n niveles recursivamente

Antes solo se soportaba flat() con profundidad 1. Ahora flat(n) aplana
hasta n niveles usando una función recursiva.

Closes #54"
```

### 4. Correr los tests

Antes de enviar el PR, asegurate de que todos los tests pasen:

```powershell
# Windows
.\run_tests.ps1

# Linux / macOS
./run_tests.sh
```

Si agregás una nueva feature, incluí al menos un test `.sz` que la ejercite en `tests/`.

### 5. Abrir el Pull Request

**Título del PR:** igual que el commit principal, claro y descriptivo.

```
fix: corregir flat(n) para profundidad mayor a 1
feature: agregar método String.repeat(n)
docs: documentar comportamiento de for-in con copies
```

**Descripción del PR**, incluí:

- **Qué hace este PR** — una o dos oraciones
- **Por qué** — qué problema resuelve o qué mejora aporta
- **Cómo probarlo** — pasos para verificar el cambio
- **Issue relacionado** — `Closes #123` si aplica

**Ejemplo:**

```markdown
## Qué hace
Corrige `flat(n)` para que aplane recursivamente n niveles en lugar de solo 1.

## Por qué
`[1, [2, [3]]].flat(2)` devolvía `[1, 2, [3]]` en lugar de `[1, 2, 3]`.

## Cómo probarlo
sz tests/unit_array_methods_edge.sz

## Issue relacionado
Closes #54
```

---

## Convenciones técnicas

- **Zero `unsafe`** — el modelo de memoria se mantiene sin bloques unsafe
- **Sin dependencias de runtime externas** — `[dependencies]` en `Cargo.toml` solo para tooling imprescindible
- **Los errores van a `stderr`** — `eprintln!` para errores, `println!` solo para output del programa y el REPL
- **Nueva sintaxis sigue el pipeline completo** — `token.rs` → `lexer.rs` → `ast.rs` → `parser.rs` → `evaluator/`
- **Nuevo operador infix requiere registro en dos lugares** en `parser.rs`: `token_precedence()` y el match `is_infix`
- **Todo nuevo bloque `{ }` debe hacer push/pop** de scope en todos los code paths, incluyendo los de error

---

## ¿Tenés dudas?

Abrí un issue con el prefijo `[QUESTION]` o comentá en el issue/PR correspondiente.
