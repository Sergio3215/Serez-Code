# ── Serez-Code Test Runner ────────────────────────────────────────────────────
# Usage:
#   .\run_tests.ps1                    # run all tests (including security)
#   .\run_tests.ps1 -filter "switch"   # run tests whose name contains "switch"
#   .\run_tests.ps1 -generate          # regenerate .expected golden files
#   .\run_tests.ps1 -unit              # only run unit_*.sz tests (using framework)
#   .\run_tests.ps1 -e2e               # only run E2E tests (numbered NN_*.sz)
#   .\run_tests.ps1 -security          # only run security tests (sec_*.sz + unit_sec_*.sz)
#   .\run_tests.ps1 -cli               # only run CLI flag, REPL, and --check mode tests
#   .\run_tests.ps1 -ai                # only run AI/ML training tests (ai_*.sz)
#   .\run_tests.ps1 -json report.json  # also write a machine-readable report
#
# ── Test types ────────────────────────────────────────────────────────────────
#
#   tests/NN_*.sz        -> E2E tests
#                          Run the file and compare stdout vs tests/NN_*.expected
#                          Use -generate to create/update the .expected file.
#
#   tests/unit_*.sz      -> Unit tests
#                          The framework (tests/framework.sz) is prepended.
#                          Each file calls test("name", () => { assert(...) })
#                          and summary() at the end.
#                          PASS = exit 0, a Results: summary and no [FAIL] line.
#                          Legacy unit_*.expected pairs are golden tests instead;
#                          they run without the framework and compare stdout.
#
#   tests/err_*.sz       -> Error tests
#                          The program must emit at least one ❌ line on stderr.
#                          PASS = non-zero exit and at least one ❌ found.
#
# ── E2E tests (tests/NN_*.sz) ─────────────────────────────────────────────────
#
#   01_arithmetic          Operadores aritméticos básicos: +, -, *, /, %, negativos
#   01_basic               Primeros pasos: out, variables, tipos primitivos
#   01_variables           Declaración y reasignación de variables
#   02_arithmetic          Aritmética con decimales y mezcla int/decimal
#   02_variables_scope     Scoping de variables: bloques, shadowing
#   02_variables           Variables tipadas, inferencia, reasignación
#   03_control_flow        if/else if/else, while, for clásico
#   03_strings             Métodos de string: length, substring, split, includes
#   04_control_flow        break/continue en while y for, for-in sobre arrays
#   04_functions           Funciones tipadas, retorno, recursión (factorial)
#   05_arrays              Arrays tipados, push/pop/shift/unshift, sort, map/filter/reduce
#   05_functions           Funciones con múltiples parámetros, lambdas, closures
#   06_arrays              Métodos avanzados de array: indexOf, join, encadenado
#   06_strings             Métodos de string: replace/replaceAll, trim, upper/lower
#   07_dicts               Dicts tipados <K,V>: acceso, insert, update, missing=null
#   08_classes             Clases: constructor, this, métodos, herencia, super
#   09_interfaces          Interfaces: definición, instanciación con {field: val}
#   10_lambdas             Lambdas, closures, currying, composición funcional
#   11_nullables           Tipos nullable T?, operador ??, null coalescing
#   12_math                Builtins matemáticos: abs, floor, ceil, round, pow, sqrt, min, max, log
#   13_edge_cases          Casos borde: overflow controlado, div/mod por cero, recursión profunda
#   14_arch_features       Features de arquitectura: arenas, scopes, watermarks
#   15_arch_stress         Stress test de memoria: muchas variables, arrays grandes, recursión
#   16_error_paths         Caminos de error: type mismatch, bounds, undeclared, stack overflow
#   17_function_syntax     Sintaxis de función: void, any, default params, Params tipados
#   18_error_cases         Casos de error adicionales en parser y evaluador
#   19_untested_docs       Features documentadas en README: reduce con string, toArray, replace vs replaceAll
#   20_more_edge_cases     Más casos borde: closures, arrays de instancias, dicts anidados
#   21_string_interp_complex Interpolación compleja: expresiones, llamadas, índices dentro de {}
#   22_math_edge           Casos borde matemáticos: sqrt(0), log(1), pow con decimales
#   23_boundary_cases      Casos límite: índices negativos, arrays vacíos, strings vacíos
#   24_chained_calls       Llamadas encadenadas: a.b().c().d(), map+filter+reduce
#   26_complex_scenarios   Escenarios complejos: banco, inventario, jerarquía de clases
#   27_escape_sequences    Secuencias de escape en strings: \n, \t, \", \\
#   28_final_checks        Verificaciones finales: todos los operadores, todos los tipos
#   29_bug_regression      Regresión de bugs corregidos: B-01 al B-30
#   30_class_regression    Regresión de clases: B-28, B-29, B-32, B-34, herencia multinivel
#   30_integral_e2e        Test integral completo: cubre todas las features de punta a punta
#   31_compound_assign     Operadores compuestos: +=, -=, *=, /=, %= en todas las formas
#   31_operator_overloading Operator overloading: op_add/sub/mul/eq/ne/lt/neg/str con Vector2D y Fraccion
#   32_e2e_full            E2E completo post-B51: primitivos, type_of, strings, arrays, dicts,
#                          for loops, switch, try/catch/finally, closures, clases, op overload,
#                          null coalescing, is type check, ternario, recursión, integrador
#   32_switch              Switch: multi-case, coerción int/decimal, strings, default
#   33_try_catch           Try/catch/finally: throw, propagación, finally override, break en catch
#   49_stdout_flush        Regresión flush de stdout (v4.0.2): 200 líneas de output
#                          verifican que el buffer se vacía completamente antes de salir
#   34_string_comprehensive Métodos de string completos: B-42, B-52, B-53, B-59 —
#                          case, trim, startsWith/endsWith, indexOf, charAt, str[i],
#                          replace(first)/replaceAll(all), substring, split, chaining
#
# ── Unit tests (tests/unit_*.sz) ──────────────────────────────────────────────
#
#   unit_arrays            Arrays: push/pop, shift/unshift, empty pop=null, indexOf/includes/contains,
#                          sort asc/desc/custom, remove(índice), join, map, map+index,
#                          filter, reduce, chain map+filter+reduce, index assign, strings, vacío dinámico
#   unit_classes           Clases: constructor, métodos de instancia, mutación de campos,
#                          herencia+override, método heredado sin override, super.method(),
#                          herencia multinivel, type_of, campos array, campos dict, array de instancias
#   unit_class_patterns    Patrones de clase: factory method (método retorna nueva instancia),
#                          Counter con reset, campo array con HOF, herencia+método nuevo,
#                          método private usado internamente, builder fluido encadenado,
#                          filter/reduce sobre array de instancias, Registry por nombre
#   unit_closures_edge     Closures avanzados: captura de valor, make_adder/multiplier,
#                          composición (compose), apply_twice, lambda como argumento,
#                          block body, map con closures independientes, currying
#   unit_closures_mutable  Closures con estado mutable: make_counter (1->2->3), dos contadores
#                          independientes, acumulador numérico, make_adder_from(n),
#                          captura de loop variable, toggle bool, acumulador de strings
#   unit_compound_assign   Operadores compuestos básicos: +=, -=, *=, /=, %= en int,
#                          += en strings, += en decimals, acumulación en loop,
#                          compound assign en arr[i], compound assign en instancia.campo
#   unit_compound_assign_edge Compound assign edge: decimals, múltiples ops en arr[i],
#                          dict[key] ops, instancia.campo ops, cadenas de assigns, loops
#   unit_control_flow      If/else if/else: simple, false, if-else, cadena, anidado, compuesto;
#                          While: básico, break, continue, anidado, false inicial;
#                          For clásico: i=i+1, i++, i--, i+=2, i*=2, break, continue, anidado, sin iter
#   unit_dict_advanced     Dict avanzado: claves int (<int,string>, <int,int>), for-in int keys,
#                          pass-by-value semántica, construcción dinámica en while (B-60 fix),
#                          tabla de frecuencias, dict desde función, dict de arrays agrupado,
#                          keys()/values() con reduce
#   unit_dict_forin        Dict for-in: iteración de keys, acceso a values, update en loop,
#                          dict vacío, keys()/values(), missing key=null, insert/update, break;
#                          toList() (array de keys), toArray() (array de pares [k,v]), dict <string,any>
#   unit_forin_string      for-in sobre string: recolecta chars en orden, cuenta caracteres,
#                          string vacío, cuenta vocales, reconstruye en mayúsculas,
#                          break al hallar char, continue salta espacios, return anticipado,
#                          itera split result, string de un solo carácter
#   unit_foreach_edge      For-in edge: return desde función, throw en loop, split result,
#                          no muta source, closures por iteración, en método de clase,
#                          ternario en body, contador ++; ternario edge: en while, interpolación,
#                          con ??, lazy; ++ edge: global, negativo, en for-in, nested while
#   unit_foreach_ternary_incr For-in básico, en orden, vacío, strings, break, continue, anidado,
#                          métodos en elementos; ternario true/false/expresión/lazy/chained/en expr/null;
#                          ++/-- postfix y prefix en while y countdown
#   unit_functions_adv     Funciones avanzadas: múltiples defaults (2 parámetros con default),
#                          recursión mutua (isEven/isOdd), recursión de cola (sumTo con acc),
#                          función que retorna función según condición, función como variable,
#                          pipeline de funciones en array, pow recursiva con exp negativo,
#                          parámetro any con dispatch por is type check
#   unit_functions         Funciones: sin tipo de retorno, string, bool, múltiples params tipados,
#                          recursión (factorial, fibonacci), early return, retorna array,
#                          retorna nullable, como argumentos, devuelve función, default params, void
#   unit_interfaces        Interfaces: campos int, mutación, string, bool, decimal,
#                          array de interfaces, en función, campo array
#   unit_is_type           is type check: primitivos, falsos cruzados; type_of: primitivos,
#                          compuestos, clases; is con herencia; type_of en reasignación; is en ternario
#   unit_lambdas           Lambdas: expresión simple, 2 params, block body, captura scope,
#                          estado entre llamadas (closure copy), currying, composición,
#                          map/filter/reduce, como argumento, array de lambdas,
#                          captura en loop, pipeline completo
#   unit_math_builtins     Math: abs (neg/pos/decimal), floor/ceil/round, pow, sqrt,
#                          min/max (negativos), log, parseInt (string/decimal/spaces),
#                          parseDecimal (string/int), composición matemática
#   unit_nullables         Nullable: ?? básico, ?? encadenado triple, función T?,
#                          ?? con retorno nullable, ?? en dict, preserva 0/false/string vacío,
#                          null en comparación, ?? en expresión compuesta
#   unit_operator_overload Operator overloading (clase Vec): op_add, op_sub, op_mul,
#                          op_eq/op_ne, op_lt por magnitud, op_neg, op_str en interpolación,
#                          op_str en array, encadenado, en if, acumulador en while,
#                          throw en op_str, herencia hereda op_add
#   unit_operators         Operadores: short-circuit && y ||, short-circuit ??,
#                          precedencia (* antes de +, comparación vs aritmética),
#                          comparación chained, negación unaria, !, igualdad strings,
#                          comparación int/decimal
#   unit_string_methods    Strings: length, toUpperCase/toLowerCase, trim, startsWith/endsWith,
#                          indexOf, charAt/str[i] out-of-bounds=null, replace(first only),
#                          replaceAll(all), substring 1-arg y 2-arg, split, includes/contains,
#                          toString en int/bool/decimal
#   unit_super_method      super.method(): sin args, no afecta own, con this fields, con args,
#                          dispatch a parent override, resultado en expresión;
#                          3-level: dispatch, own override, chained, this.value via super
#   unit_switch            Switch: match int exacto, match string, default, multi-value case,
#                          sin match sin default, expresión como valor, en función con return, bool
#   unit_switch_edge       Switch edge: sin fall-through, decimals, null, en for loop,
#                          anidado, throw en case, break en case rompe for, default una vez,
#                          multi-value case (valor en medio)
#   unit_try_catch         Try/catch: recibe string/int, código post-throw no ejecuta,
#                          finally en path normal y throw, propagación desde función,
#                          try anidado inner catch y rethrow, catch con return, assert
#   unit_try_catch_edge    Try edge: return preservado through finally, throw en finally overridea,
#                          throw en for/while loop, finally-only modifica vars,
#                          catch body throws, nested rethrow chain, propagación multicall;
#                          break en catch sale del for (B-54), continue en catch salta iter (B-54),
#                          throw en for-init se captura (B-55)
#
# ── Tests for improve-branch features (tests/unit_*.sz) ───────────────────────
#
#   unit_bitwise_ops       Bitwise operators: & | ^ ~ << >> including flag manipulation,
#                          boundary shifts, sign-extending >>, ops in expressions
#   unit_do_while          do/while loops: body-runs-once-on-false, counting, factorial,
#                          break/continue, nested, return-inside, vs while equivalence
#   unit_lexer_edge        Lexer/parser edge cases: ++ --, hex/binary literals, escape seqs,
#                          string interpolation, spread (...), arrow =>, keyword-as-method
#   unit_logical_operators && y || devuelven un OPERANDO; la regla única de falsy
#                          (incluye colecciones vacías); corto circuito; el prefijo !
#   unit_nested_receiver_writeback
#                          Un método PROPIO sobre un receptor anidado (a[i].m(),
#                          o.campo.m(), this.celdas[i].m()) persiste sus mutaciones
#   unit_nested_assignment a.b.c = x y a[i][j] = x; setter en el medio del camino;
#                          escribir sobre un temporal sigue siendo error
#   unit_optional_chain    Optional chaining (?.) with Node/Container classes: null vs non-null,
#                          chained ?., with ??, in conditionals, describe method
#   unit_power_op          Power operator (**): int**int, negative exponent→decimal,
#                          decimal, mixed, in expressions, vs recursive ipow
#   unit_scope_shadow      Scope/ScopeStack: block scope, shadowing multiple levels,
#                          assignment-modifies-outer, for-loop scoping, named fn vs arrow
#                          closure capture semantics, class method local-shadows-field
#   unit_static_methods    Static methods: ClassName.method(), calling another static,
#                          factory pattern (Counter.zero/from), multiple instances,
#                          StringUtils helpers
#
# ── AI / ML tests (tests/ai_*.sz) ─────────────────────────────────────────────
#
#   ai_*.sz              -> AI training integration tests (run with -ai or all)
#                          Framework-based tests for: gradient descent convergence,
#                          training loop behavior, autodiff through attention layers.
#
#   ai_attention_training  Full attention training: Embedding+MHA+LN+Dense,
#                          binary classification, verifies loss decreases, predictions
#                          correct, and gradients flow through all layers.
#
# ── Security tests ────────────────────────────────────────────────────────────
#
#   tests/sec_*.sz       -> Security error tests (run with -security or all)
#                          Must emit at least one ❌ on stderr.
#                          Tests: stack overflow, private access, OOB, null deref,
#                          type violations, overflow, div-by-zero, undeclared vars,
#                          bad shifts, undeclared classes, non-function call.
#
#   tests/unit_sec_*.sz  -> Security unit tests (run with -security or all)
#                          Framework-based tests verifying safe behavior:
#                          arithmetic safety, type safety, null safety,
#                          error isolation, resource limits, injection prevention.
#
# ── Error tests (tests/err_*.sz) ──────────────────────────────────────────────
#
#   err_arity              Llamar función con número incorrecto de argumentos
#   err_bool_plus_int      Suma bool + int (type mismatch en operador)
#   err_bounds             Acceso a array fuera de rango (índice negativo o >= length)
#   err_call_undefined     Llamar identificador no declarado como función
#   err_div_zero           División entera por cero
#   err_extra_iface_field  Interface instanciada con campo extra no declarado
#   err_for_scope_leak     Variable de for-loop usada fuera de su scope
#   err_foreach_dict       for-in sobre un valor no iterable (bool)
#   err_foreach_nonarray   for-in sobre un entero (no iterable)
#   err_modulo_zero        Operador % con divisor cero
#   err_not_function       Llamar un valor que no es función (e.g. un int)
#   err_overflow           Desbordamiento aritmético en int (i64::MAX + 1)
#   err_parse_incomplete_expr  Expresión cortada (`return a +`) reporta error de parseo
#   err_parse_let_noname   `let = 5;` sin nombre de variable reporta error de parseo
#   err_parse_let_novalue  `let x = ;` sin valor reporta error de parseo
#   err_parse_named_typed_arrow  `int f(int n) => {}` (forma inválida) reporta error de parseo
#   err_private            Acceso a campo privado de clase desde fuera
#   err_return_toplevel    return en nivel superior (fuera de función)
#   err_return_type_mismatch Función declarada int devuelve string
#   err_sort_mixed         sort sobre array con tipos mezclados (int y string)
#   err_throw_nested_arg   throw al evaluar argumento anidado f(g()) NO muere en silencio
#   err_throw_out_stmt     throw en `out f()` conserva el mensaje (no "Referencia inválida")
#   err_type_param         Parámetro tipado recibe tipo incorrecto
#   err_typed_push         push de tipo incorrecto en array tipado [int]
#   err_undeclared         Leer variable no declarada
#   err_undeclared_assign  Asignar variable no declarada (sin let)
#   err_undeclared_class   new de clase no definida
#
# ── CLI / REPL / --check Tests (-cli) ────────────────────────────────────────
#
#   CLI flag tests:  --version, unknown flags, non-.sz extension, missing file
#   Package mgr:     sz install (serez.json), sz install pkg@ver, sz uninstall,
#                    sz uninstall nonexistent (error)
#   REPL tests:      piped stdin; arithmetic, strings, variable persistence,
#                    function definition+call, error recovery across lines
#   --check tests:   Flash Scope Criticality output, Estimated Global Memory,
#                    missing file error
#
# Exit code: 0 = all passed, 1 = failures found
# ─────────────────────────────────────────────────────────────────────────────

param(
    [string]$filter    = "",
    [switch]$generate  = $false,
    [switch]$unit      = $false,
    [switch]$e2e       = $false,
    [switch]$security  = $false,
    [switch]$cli       = $false,
    [switch]$ai        = $false,
    [string]$json      = ""
)

$ErrorActionPreference = "Stop"
$startedAt = Get-Date
$root       = $PSScriptRoot
$testsDir   = Join-Path $root "tests"
$framework  = Join-Path $testsDir "framework.sz"
$binary     = Join-Path $root "target\release\sz.exe"
# Unique per run. A fixed name meant two runs on the same checkout — or one
# lingering handle from a previous run — collided on Set-Content and aborted the
# whole suite mid-way with "used by another process". run_tests.sh already
# suffixed with $$; this is the same fix.
$tempFile   = Join-Path $testsDir "~unit_temp_$PID.sz"

# Expose project root as SEREZ_HOME so `import "std/..."` resolves correctly
$env:SEREZ_HOME = $root
# Expose tests/packages as SEREZ_PACKAGES so package tests can import local packages
$env:SEREZ_PACKAGES = Join-Path $root "tests\packages"

# ── Fixture preflight ─────────────────────────────────────────────────────────
# These trees are loaded by the import/export, package and runner-integrity
# tests. They were excluded by `.gitignore` for a long time, so a fresh clone
# had none of them and eight tests failed with "ModuleNotFound" — a message that
# points at the language, not at the missing checkout. Fail here instead.
$requiredFixtures = @(
    @{ Path = "tests/lib/greet.sz";        Used = "unit_sec_import, unit_import, 46_import_e2e" },
    @{ Path = "tests/lib/math_utils.sz";   Used = "unit_import, unit_export, 47_export_e2e, sec_export" },
    @{ Path = "tests/packages/serez.json"; Used = "unit_packages, 55_packages_e2e (via SEREZ_PACKAGES)" },
    @{ Path = "tests/runner_fixtures/unit_abort_before_summary.sz"; Used = "runner integrity check" },
    @{ Path = "std/result.sz";             Used = "unit_stdlib_*, 48_stdlib_e2e (via SEREZ_HOME)" },
    @{ Path = "std/iter.sz";               Used = "unit_stdlib_iter, unit_generators, 50_generators_e2e" }
)
$missingFixtures = @($requiredFixtures | Where-Object { -not (Test-Path (Join-Path $root $_.Path)) })
if ($missingFixtures.Count -gt 0) {
    Write-Host "Missing test fixtures — this checkout cannot produce a valid result:" -ForegroundColor Red
    $missingFixtures | ForEach-Object {
        Write-Host "  $($_.Path)  (needed by $($_.Used))" -ForegroundColor Yellow
    }
    Write-Host "These files must be tracked in git. Check .gitignore." -ForegroundColor Red
    exit 1
}

# ── Build first ───────────────────────────────────────────────────────────────
Write-Host "Building..." -ForegroundColor Cyan
Push-Location $root
$buildOut = cargo build --release 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED:" -ForegroundColor Red
    $buildOut | Write-Host
    exit 1
}
Pop-Location
Write-Host "Build OK`n" -ForegroundColor Green

# ── Helpers ───────────────────────────────────────────────────────────────────
$pass = 0
$fail = 0
$skip = 0

# Every counted outcome is also recorded, so the run can be read by something
# other than a human scrolling a terminal. `-json <path>` writes it out; with
# no path the list is still built and used for the self-check below, which is
# what keeps the recorder honest: if a site increments a counter without
# recording, the totals stop matching and the run fails.
$script:results  = New-Object System.Collections.Generic.List[object]
$script:category = "startup"

function Add-Result([string]$status, [string]$label, [string]$detail = "") {
    switch ($status) {
        "pass" { $script:pass++ }
        "fail" { $script:fail++ }
        "skip" { $script:skip++ }
        default { throw "Add-Result: unknown status '$status'" }
    }
    $script:results.Add([ordered]@{
        name     = $label
        category = $script:category
        status   = $status
        detail   = $detail
    })
}

function Invoke-Sz([string]$runFile) {
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    $process = Start-Process -FilePath $binary -ArgumentList "`"$runFile`"" `
        -NoNewWindow -Wait `
        -PassThru `
        -RedirectStandardOutput $outFile `
        -RedirectStandardError  $errFile
    $stdout = if (Test-Path $outFile) { Get-Content $outFile } else { @() }
    $stderr = if (Test-Path $errFile) { Get-Content $errFile } else { @() }
    Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue
    return @{ stdout = $stdout; stderr = $stderr; exitCode = $process.ExitCode }
}

function Get-UnitFailureReason($result) {
    $failures = @($result.stdout | Where-Object { $_ -match "^\[FAIL\]" })
    $summary = @($result.stdout | Where-Object { $_ -match "^Results:" })
    if ($result.exitCode -ne 0) { return "process exited with code $($result.exitCode)" }
    if ($failures.Count -gt 0) { return "framework reported $($failures.Count) failure(s)" }
    if ($summary.Count -eq 0) { return "missing Results: summary" }
    return $null
}

function Run-Test([string]$label, [string]$file, [string]$expectedFile, [bool]$isUnit, [bool]$isErr) {
    if ($filter -and $label -notlike "*$filter*") { return }

    if ($isUnit) {
        $fw  = Get-Content $framework -Raw
        $src = Get-Content $file -Raw
        Set-Content $tempFile ($fw + "`n" + $src) -NoNewline
        $runFile = $tempFile
    } else {
        $runFile = $file
    }

    $result = Invoke-Sz $runFile
    $stdout = $result.stdout
    $stderr = $result.stderr

    if ($isErr) {
        # Error tests must fail as a process and emit a diagnostic on stderr.
        $hasError = ($stderr | Where-Object { $_ -match "^❌" }).Count -gt 0
        if ($result.exitCode -ne 0 -and $hasError) {
            Write-Host "[PASS] $label" -ForegroundColor Green
            Add-Result "pass" $label
        } else {
            Write-Host "[FAIL] $label — expected non-zero exit and an error diagnostic (exit $($result.exitCode))" -ForegroundColor Red
            Add-Result "fail" $label "expected non-zero exit and an error diagnostic (exit $($result.exitCode))"
        }
        return
    }

    if ($isUnit) {
        # Unit tests must finish normally and prove the framework reached summary().
        $failures = @($stdout | Where-Object { $_ -match "^\[FAIL\]" })
        $summary = @($stdout | Where-Object { $_ -match "^Results:" })
        $reason = Get-UnitFailureReason $result
        if ($null -eq $reason) {
            Write-Host "[PASS] $label" -ForegroundColor Green
            if ($summary) { Write-Host "       $summary" -ForegroundColor Gray }
            Add-Result "pass" $label "$summary"
        } else {
            Write-Host "[FAIL] $label — $reason" -ForegroundColor Red
            $failures | ForEach-Object { Write-Host "       $_" -ForegroundColor Yellow }
            if ($result.exitCode -ne 0) {
                $stderr | Select-Object -First 3 | ForEach-Object {
                    Write-Host "       $_" -ForegroundColor Yellow
                }
            }
            Add-Result "fail" $label $reason
        }
        return
    }

    # E2E programs must complete before output can be accepted or generated.
    if ($result.exitCode -ne 0) {
        Write-Host "[FAIL] $label — process exited with code $($result.exitCode)" -ForegroundColor Red
        $stderr | Select-Object -First 3 | ForEach-Object {
            Write-Host "       $_" -ForegroundColor Yellow
        }
        Add-Result "fail" $label "process exited with code $($result.exitCode)"
        return
    }

    # E2E golden file test
    if ($generate) {
        $stdout | Set-Content $expectedFile
        Write-Host "[GEN]  $label -> $expectedFile" -ForegroundColor Cyan
        return
    }

    if (-not (Test-Path $expectedFile)) {
        Write-Host "[SKIP] $label (no .expected file — run with -generate to create)" -ForegroundColor Yellow
        Add-Result "skip" $label "no .expected file"
        return
    }

    $expected = Get-Content $expectedFile
    $actual   = $stdout

    if ($null -eq $actual) { $actual = @() }
    if ($null -eq $expected) { $expected = @() }

    $diff = Compare-Object $expected $actual
    if ($null -eq $diff) {
        Write-Host "[PASS] $label" -ForegroundColor Green
        Add-Result "pass" $label
    } else {
        Write-Host "[FAIL] $label" -ForegroundColor Red
        $diff | ForEach-Object {
            $arrow = if ($_.SideIndicator -eq "<=") { "expected:" } else { "  actual:" }
            Write-Host "       $arrow $($_.InputObject)" -ForegroundColor Yellow
        }
        Add-Result "fail" $label "stdout differs from the golden file"
    }
}

# ── CLI / REPL / check-mode test helper ──────────────────────────────────────
function Invoke-Binary([string[]]$binArgs, [string]$stdinContent = "", [string]$workDir = "") {
    if ($workDir -eq "") { $workDir = $root }
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    if ($stdinContent -ne "") {
        $inFile  = [System.IO.Path]::GetTempFileName()
        [System.IO.File]::WriteAllText($inFile, $stdinContent)
        Start-Process -FilePath $binary -ArgumentList $binArgs `
            -WorkingDirectory $workDir `
            -NoNewWindow -Wait `
            -RedirectStandardInput  $inFile `
            -RedirectStandardOutput $outFile `
            -RedirectStandardError  $errFile
        Remove-Item $inFile -ErrorAction SilentlyContinue
    } else {
        Start-Process -FilePath $binary -ArgumentList $binArgs `
            -WorkingDirectory $workDir `
            -NoNewWindow -Wait `
            -RedirectStandardOutput $outFile `
            -RedirectStandardError  $errFile
    }
    $stdout = if (Test-Path $outFile) { (Get-Content $outFile -Raw) } else { "" }
    $stderr = if (Test-Path $errFile) { (Get-Content $errFile -Raw) } else { "" }
    Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue
    return @{ stdout = ($stdout ?? ""); stderr = ($stderr ?? "") }
}

function Run-CLI-Test([string]$label, [string[]]$binArgs, [string]$expectOut = "",
                      [string]$expectErr = "", [string]$stdinContent = "",
                      [string]$workDir = "") {
    if ($filter -and $label -notlike "*$filter*") { return }
    $r = Invoke-Binary $binArgs $stdinContent $workDir
    $ok = $true; $reason = ""
    if ($expectOut -ne "" -and $r.stdout -notmatch [regex]::Escape($expectOut)) {
        $ok = $false; $reason = "stdout missing '$expectOut'"
    }
    if ($expectErr -ne "" -and $r.stderr -notmatch [regex]::Escape($expectErr)) {
        $ok = $false; $reason = "stderr missing '$expectErr'"
    }
    if ($ok) { Write-Host "[PASS] $label" -ForegroundColor Green; Add-Result "pass" $label }
    else     { Write-Host "[FAIL] $label — $reason" -ForegroundColor Red; Add-Result "fail" $label $reason }
}

function Run-Repl-Test([string]$label, [string]$expectOut, [string]$forbidOut,
                      [string]$expectErr, [string]$stdinFixture) {
    # These cases can only be stated as an absence: that a line the parser
    # rejected did NOT run, and that a line the process could not decode did NOT
    # kill the session. Run-CLI-Test asserts containment only, which is why
    # neither defect was visible to the five REPL cases that already existed.
    # The input comes from a fixture file because one of them is deliberately
    # not UTF-8 and cannot survive being carried as a string.
    if ($filter -and $label -notlike "*$filter*") { return }
    $inFile  = Join-Path $testsDir "runner_fixtures\$stdinFixture"
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    Start-Process -FilePath $binary -WorkingDirectory $root `
        -NoNewWindow -Wait `
        -RedirectStandardInput  $inFile `
        -RedirectStandardOutput $outFile `
        -RedirectStandardError  $errFile
    $out = if (Test-Path $outFile) { (Get-Content $outFile -Raw) } else { "" }
    $err = if (Test-Path $errFile) { (Get-Content $errFile -Raw) } else { "" }
    Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue
    $out = ($out ?? ""); $err = ($err ?? "")

    $ok = $true; $reason = ""
    if ($expectOut -ne "" -and $out -notmatch [regex]::Escape($expectOut)) {
        $ok = $false; $reason = "stdout missing '$expectOut'"
    }
    if ($forbidOut -ne "" -and $out -match [regex]::Escape($forbidOut)) {
        $ok = $false; $reason = "stdout contains '$forbidOut', which must not have run"
    }
    if ($expectErr -ne "" -and $err -notmatch [regex]::Escape($expectErr)) {
        $ok = $false; $reason = "stderr missing '$expectErr'"
    }
    if ($ok) { Write-Host "[PASS] $label" -ForegroundColor Green; Add-Result "pass" $label }
    else     { Write-Host "[FAIL] $label - $reason" -ForegroundColor Red; Add-Result "fail" $label $reason }
}

# The runner itself must reject a unit program that aborts before summary().
Write-Host "═══ Test Runner Integrity ════════════════════" -ForegroundColor Cyan
$script:category = "runner-integrity"
$runnerFixture = Join-Path $testsDir "runner_fixtures\unit_abort_before_summary.sz"
$fw = Get-Content $framework -Raw
$src = Get-Content $runnerFixture -Raw
Set-Content $tempFile ($fw + "`n" + $src) -NoNewline
$runnerProbe = Invoke-Sz $tempFile
$runnerReason = Get-UnitFailureReason $runnerProbe
# A non-zero exit with no summary is not enough on its own: a path the binary
# cannot open satisfies both, which is how the bash runner reported PASS here
# for a file-read error rather than for the abort this guard exists to detect.
# Require the fixture's own diagnostic, so it can only pass by actually running.
$runnerDiag = @($runnerProbe.stderr | Where-Object { $_ -match "SZ4004" }).Count
if ($null -ne $runnerReason -and $runnerProbe.exitCode -ne 0 -and $runnerDiag -gt 0) {
    Write-Host "[PASS] runner rejects abort before summary" -ForegroundColor Green
    Add-Result "pass" "runner rejects abort before summary"
} else {
    $reason = "the runner accepted a suite that aborted before summary()"
    if ($runnerDiag -eq 0) {
        $reason = "the fixture never reached the interpreter: $($runnerProbe.stderr | Select-Object -First 1)"
    }
    Write-Host "[FAIL] runner rejects abort before summary — $reason" -ForegroundColor Red
    Add-Result "fail" "runner rejects abort before summary" $reason
}
Write-Host ""

# ── Discover and run tests ────────────────────────────────────────────────────
$runAll  = -not $unit -and -not $e2e -and -not $security -and -not $cli -and -not $ai

Write-Host "═══ E2E Tests ════════════════════════════════" -ForegroundColor Cyan
$script:category = "e2e"
if ($runAll -or $e2e) {
    Get-ChildItem $testsDir -Filter "*.sz" |
        Where-Object { $_.Name -match "^\d{2}_" } |
        Sort-Object Name | ForEach-Object {
            $label    = $_.BaseName
            $expected = Join-Path $testsDir ($_.BaseName + ".expected")
            Run-Test $label $_.FullName $expected $false $false
        }
}

Write-Host ""
Write-Host "═══ Unit Tests ═══════════════════════════════" -ForegroundColor Cyan
$script:category = "unit"
if ($runAll -or $unit) {
    Get-ChildItem $testsDir -Filter "unit_*.sz" |
        Where-Object { $_.Name -notmatch "^unit_sec_" } |
        Sort-Object Name | ForEach-Object {
            $label = $_.BaseName
            $expected = Join-Path $testsDir ($_.BaseName + ".expected")
            if (Test-Path $expected) {
                Run-Test $label $_.FullName $expected $false $false
            } else {
                Run-Test $label $_.FullName "" $true $false
            }
        }
}

Write-Host ""
Write-Host "═══ Error Tests ══════════════════════════════" -ForegroundColor Cyan
$script:category = "error"
if ($runAll -or $e2e) {
    Get-ChildItem $testsDir -Filter "err_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $false $true
    }
}

Write-Host ""
Write-Host "═══ Security Tests ═══════════════════════════" -ForegroundColor Cyan
$script:category = "security"
if ($runAll -or $security) {
    # sec_*.sz — must emit ❌ (runtime error tests)
    Get-ChildItem $testsDir -Filter "sec_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $false $true
    }
    # unit_sec_*.sz — framework-based safety tests
    Get-ChildItem $testsDir -Filter "unit_sec_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $true $false
    }
}

# ── AI Tests ──────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══ AI Tests ═════════════════════════════════" -ForegroundColor Cyan
$script:category = "ai"
if ($runAll -or $ai) {
    # ai_*.sz — framework-based tests for AI/ML training loops and autodiff behavior
    Get-ChildItem $testsDir -Filter "ai_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $true $false
    }
}

# ── Rust Unit Tests ───────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══ Rust Unit Tests ══════════════════════════" -ForegroundColor Cyan
$script:category = "rust"
if ($runAll -or $unit) {
    foreach ($mod in @(
        @{ filter = "package_manager::tests";                 label = "package_manager unit tests" },
        @{ filter = "evaluator::namespaces_gui::css::tests";  label = "css nativo: condiciones and/or/not + bloques @when/@else" }
    )) {
        # -filter applies here too. run_tests.sh already skipped these under a
        # filter while this runner ran them regardless, so a filtered run
        # reported different totals on the two platforms.
        if ($filter -and $mod.label -notlike "*$filter*") { continue }
        Push-Location $root
        $cargoOut = cargo test $mod.filter 2>&1
        $cargoOk  = $LASTEXITCODE -eq 0
        Pop-Location
        # `cargo test <filter>` exits 0 when the filter matches nothing, so a
        # renamed or misspelled module would report PASS while asserting
        # nothing. Require that tests actually ran.
        $ran = 0
        $cargoOut | ForEach-Object {
            if ($_ -match 'test result: ok\. (\d+) passed') { $ran += [int]$Matches[1] }
        }
        if ($cargoOk -and $ran -gt 0) {
            Write-Host "[PASS] $($mod.label) ($ran tests)" -ForegroundColor Green
            Add-Result "pass" $mod.label "$ran tests"
        } elseif ($cargoOk) {
            Write-Host "[FAIL] $($mod.label) — filter '$($mod.filter)' matched no tests" -ForegroundColor Red
            Add-Result "fail" $mod.label "filter '$($mod.filter)' matched no tests"
        } else {
            Write-Host "[FAIL] $($mod.label)" -ForegroundColor Red
            $cargoOut | Where-Object { $_ -match "FAILED|panicked|error" } |
                ForEach-Object { Write-Host "       $_" -ForegroundColor Yellow }
            Add-Result "fail" $mod.label "cargo test reported failures"
        }
    }
}

# ── CLI Tests ─────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══ CLI Tests ════════════════════════════════" -ForegroundColor Cyan
$script:category = "cli"
if ($runAll -or $cli) {
    Run-CLI-Test "cli: --version prints version"       @("--version") `
                 -expectOut "Serez-Code v"
    Run-CLI-Test "cli: --help prints usage on stdout"  @("--help") `
                 -expectOut "USAGE"
    Run-CLI-Test "cli: -h is accepted"                 @("-h") `
                 -expectOut "USAGE"
    Run-CLI-Test "cli: help subcommand is accepted"    @("help") `
                 -expectOut "USAGE"
    Run-CLI-Test "cli: --help documents the exit codes" @("--help") `
                 -expectOut "EXIT CODES"
    Run-CLI-Test "cli: unknown flag reports error"     @("--unknown-flag") `
                 -expectErr "Unknown flag"
    Run-CLI-Test "cli: unknown flag points at --help"  @("--unknown-flag") `
                 -expectErr "sz --help"
    Run-CLI-Test "cli: no file argument points at --help" @("--check") `
                 -expectErr "sz --help"
    Run-CLI-Test "cli: non-.sz file rejected"          @("readme.txt") `
                 -expectErr ".sz extension"
    $noSz = "`"$(Join-Path $testsDir 'this_file_does_not_exist.sz')`""
    Run-CLI-Test "cli: missing .sz file reports error" @($noSz) `
                 -expectErr "ERROR reading file"
    Run-CLI-Test "cli: --watch on a missing file reports, not panics" `
                 @("--watch", "nosuchfile_for_the_watch_test.sz") `
                 -expectErr "cannot watch"
    # Regresión throw 2026-07-14: el mensaje debe llegar ÍNTEGRO a stderr
    # (antes: "Referencia inválida" en `out f()`, y silencio total en f(g())).
    $thrOut = "`"$(Join-Path $testsDir 'err_throw_out_stmt.sz')`""
    Run-CLI-Test "cli: uncaught throw en out f() conserva el mensaje" @($thrOut) `
                 -expectErr "boom out con local 7"
    $thrArg = "`"$(Join-Path $testsDir 'err_throw_nested_arg.sz')`""
    Run-CLI-Test "cli: throw en argumento anidado no muere en silencio" @($thrArg) `
                 -expectErr "desde inner"
}

# ── --eval Tests ──────────────────────────────────────────────────────────────
# `sz --eval` runs a snippet with no file behind it: no serez.json, so no
# permissions, and lockdown on. Same pipeline as `sz file.sz` (see src/run.rs) —
# these cover the door, not the interpreter.
Write-Host ""
Write-Host "═══ --eval Tests ═════════════════════════════" -ForegroundColor Cyan
$script:category = "eval"
if ($runAll -or $cli) {
    Run-CLI-Test "eval: runs a snippet from argv"       @("--eval", "`"out 2+3;`"") `
                 -expectOut "5"
    Run-CLI-Test "eval: reads the snippet from stdin"   @("--eval", "-") `
                 -expectOut "100" -stdinContent "let x = 10;`nout x * x;"
    Run-CLI-Test "eval: -e is accepted as a short form" @("-e", "`"out 7;`"") `
                 -expectOut "7"
    Run-CLI-Test "eval: no snippet reports usage"       @("--eval") `
                 -expectErr "Usage: sz --eval"
    Run-CLI-Test "eval: parse errors still abort"       @("--eval", "-") `
                 -expectErr "Aborted" -stdinContent "let = ;"

    # ── Lockdown ──────────────────────────────────────────────────────────────
    # The permission set is a manifest, not a sandbox. Everything below reaches
    # the machine without any permission being declared, so lockdown closes it.
    Run-CLI-Test "eval/lockdown: use permissions denied" @("--eval", "-") `
                 -expectErr "use permissions" `
                 -stdinContent "use permissions { OS };`nout 1;"
    Run-CLI-Test "eval/lockdown: File denied"            @("--eval", "-") `
                 -expectErr "File is not available" `
                 -stdinContent "out File.read(`"Cargo.toml`");"
    Run-CLI-Test "eval/lockdown: import denied"          @("--eval", "-") `
                 -expectErr "import is not available" `
                 -stdinContent "import `"std/math`";"
    Run-CLI-Test "eval/lockdown: URL import denied"      @("--eval", "-") `
                 -expectErr "import is not available" `
                 -stdinContent "import `"https://example.invalid/x.sz`";"
    Run-CLI-Test "eval/lockdown: Autodiff weights denied" @("--eval", "-") `
                 -expectErr "Autodiff.saveWeights" `
                 -stdinContent "Autodiff.saveWeights(`"w.szw`", []);"
    Run-CLI-Test "eval/lockdown: permission set is empty" @("--eval", "-") `
                 -expectErr "requires permission" `
                 -stdinContent "unsafe { OS.exec(`"whoami`"); }"
    # Deliberately NOT gated: in the wasm build `fetch` runs in the viewer's own
    # tab under the browser's origin rules. Reaching the arity error proves the
    # builtin is still live under lockdown (and needs no network to check).
    Run-CLI-Test "eval/lockdown: fetch is NOT gated"     @("--eval", "-") `
                 -expectErr "fetch(url," `
                 -stdinContent "out fetch();"
    # Lockdown is only for `--eval`; running your own file keeps declaring inline.
    $permFile = Join-Path $env:TEMP "sz_eval_perm_$(Get-Random).sz"
    Set-Content $permFile "use permissions { Time };`nout DateTime.now() != null;" -NoNewline
    Run-CLI-Test "eval/lockdown: sz file.sz still grants inline" @("`"$permFile`"") `
                 -expectOut "true"
    Remove-Item $permFile -ErrorAction SilentlyContinue
}

# ── REPL Tests ────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══ REPL Tests ═══════════════════════════════" -ForegroundColor Cyan
$script:category = "repl"
if ($runAll -or $cli) {
    Run-CLI-Test "repl: arithmetic output"              @() `
                 -expectOut "5"        -stdinContent "out 2+3;"
    Run-CLI-Test "repl: string output"                  @() `
                 -expectOut "hello"    -stdinContent "out `"hello`";"
    Run-CLI-Test "repl: variable persists across lines" @() `
                 -expectOut "42"       -stdinContent "let x = 42;`nout x;"
    Run-CLI-Test "repl: function defined and called"    @() `
                 -expectOut "12"       -stdinContent "fn int add(int a, int b) { return a + b; }`nout add(5, 7);"
    Run-CLI-Test "repl: error recovery continues"       @() `
                 -expectOut "survived" -expectErr "❌" `
                 -stdinContent "out undefined_xyz_var;`nout `"survived`";"
    Run-Repl-Test "repl: a parse error does not run the line" `
                  -expectOut "the session continues" `
                  -forbidOut "SIDE_EFFECT_RAN" `
                  -expectErr "Aborted: fix the parse errors" `
                  -stdinFixture "repl_parse_error.txt"
    Run-Repl-Test "repl: a parse error shows the source and caret" `
                  -expectOut "the session continues" `
                  -expectErr "let x = ;" `
                  -stdinFixture "repl_parse_error.txt"
    Run-Repl-Test "repl: a non-UTF-8 line is skipped, not fatal" `
                  -expectOut "after the bad line" `
                  -expectErr "did not contain valid UTF-8" `
                  -stdinFixture "repl_invalid_utf8.txt"
    Run-CLI-Test "repl: without a grant a namespace is denied" @() `
                 -expectErr "SZ6001" `
                 -stdinContent "out DateTime.now();"
    Run-CLI-Test "repl: a grant persists across lines" @() `
                 -expectOut "true" `
                 -stdinContent "use permissions { Time }`nout DateTime.now() != null;`nout OS.platform();"
    Run-CLI-Test "repl: a grant opens only what it names" @() `
                 -expectOut "true" -expectErr "requires permission 'OS'" `
                 -stdinContent "use permissions { Time }`nout DateTime.now() != null;`nout OS.platform();"
}

# ── --check Mode Tests ────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══ --check Mode Tests ═══════════════════════" -ForegroundColor Cyan
$script:category = "check"
if ($runAll -or $cli) {
    $chk    = "`"$(Join-Path $testsDir '01_basic.sz')`""
    $noChk  = "`"$(Join-Path $testsDir 'no_such_check_file.sz')`""
    Run-CLI-Test "check: Flash Scope Criticality header" @("--check", $chk) `
                 -expectOut "Flash Scope Criticality"
    Run-CLI-Test "check: Estimated Global Memory line"   @("--check", $chk) `
                 -expectOut "Estimated Global Memory"
    Run-CLI-Test "check: missing file reports error"     @("--check", $noChk) `
                 -expectErr "ERROR reading file"
}

# ── Package Manager CLI Tests ─────────────────────────────────────────────────
Write-Host ""
Write-Host "═══ Package Manager Tests ════════════════════" -ForegroundColor Cyan
$script:category = "package-manager"
if ($runAll -or $cli) {
    $tmpProject = Join-Path $env:TEMP "sz_pkg_test_$(Get-Random)"
    New-Item -ItemType Directory -Force $tmpProject | Out-Null
    Set-Content (Join-Path $tmpProject "serez.json") `
        '{"name":"test-project","version":"1.0.0","dependencies":{"test-pkg":"1.0.0"}}' -NoNewline
    $env:SEREZ_REGISTRY = Join-Path $root "tests\registry"

    Run-CLI-Test "cli: sz install reads serez.json"     @("install") `
                 -expectOut "Installed test-pkg"  -workDir $tmpProject
    Run-CLI-Test "cli: sz install pkg@ver explicit"     @("install", "test-pkg@1.0.0") `
                 -expectOut "Installed test-pkg"  -workDir $tmpProject

    # Filtered runs must not inherit state from a test that the filter skipped.
    # Arrange the uninstall precondition without counting it as a separate test.
    $uninstallLabel = "cli: sz uninstall removes package"
    if (-not $filter -or $uninstallLabel -like "*$filter*") {
        $null = Invoke-Binary @("install", "test-pkg@1.0.0") "" $tmpProject
    }
    Run-CLI-Test "cli: sz uninstall removes package"    @("uninstall", "test-pkg") `
                 -expectOut "Uninstalled test-pkg" -workDir $tmpProject
    Run-CLI-Test "cli: sz uninstall nonexistent errors" @("uninstall", "test-pkg") `
                 -expectErr "not installed"        -workDir $tmpProject

    $env:SEREZ_REGISTRY = ""
    Remove-Item $tmpProject -Recurse -Force -ErrorAction SilentlyContinue

    # ── runtime requirement ───────────────────────────────────────────────────
    # `serez-code` in "dependencies" declares the minimum runtime, not a package
    # to fetch. serez-ui declares it; before the key was reserved, `sz install`
    # in that project failed on the space and the '>' in ">= 9.17.0".
    $tmpFloor = Join-Path $env:TEMP "sz_floor_test_$(Get-Random)"
    New-Item -ItemType Directory -Force $tmpFloor | Out-Null

    Set-Content (Join-Path $tmpFloor "serez.json") `
        '{"name":"floor-ok","version":"1.0.0","dependencies":{"serez-code":">= 0.1.0"}}' -NoNewline
    Run-CLI-Test "cli: satisfied runtime requirement installs cleanly" @("install") `
                 -expectOut "runtime requirement satisfied" -workDir $tmpFloor

    Set-Content (Join-Path $tmpFloor "serez.json") `
        '{"name":"floor-bad","version":"1.0.0","dependencies":{"serez-code":">= 999.0.0"}}' -NoNewline
    Run-CLI-Test "cli: unsatisfiable runtime requirement is reported" @("install") `
                 -expectErr "requires Serez Code >= 999.0.0" -workDir $tmpFloor

    Run-CLI-Test "cli: the runtime is not an installable package" @("install", "serez-code") `
                 -expectErr "is the runtime" -workDir $tmpFloor

    Remove-Item $tmpFloor -Recurse -Force -ErrorAction SilentlyContinue

    # ── sz init tests ─────────────────────────────────────────────────────────
    $tmpInit = Join-Path $env:TEMP "sz_init_test_$(Get-Random)"
    New-Item -ItemType Directory -Force $tmpInit | Out-Null

    Run-CLI-Test "cli: sz init --y creates serez.json"   @("init", "--y") `
                 -expectOut "Created serez.json"  -workDir $tmpInit

    $initLabel = "cli: sz init --y serez.json has name/scripts/dev"
    if (-not $filter -or $initLabel -like "*$filter*") {
        $initJson = Get-Content (Join-Path $tmpInit "serez.json") -Raw -ErrorAction SilentlyContinue
        if ($initJson -and $initJson -match '"name"' -and $initJson -match '"scripts"' -and $initJson -match '"dev"') {
            Write-Host "[PASS] $initLabel" -ForegroundColor Green
            Add-Result "pass" $initLabel
        } else {
            Write-Host "[FAIL] $initLabel" -ForegroundColor Red
            Add-Result "fail" $initLabel "serez.json is missing name/scripts/dev"
        }
    }

    Run-CLI-Test "cli: sz init --y overwrites existing serez.json" @("init", "--y") `
                 -expectOut "Created serez.json"  -workDir $tmpInit

    Remove-Item $tmpInit -Recurse -Force -ErrorAction SilentlyContinue

    # ── sz run tests ──────────────────────────────────────────────────────────
    $tmpRun = Join-Path $env:TEMP "sz_run_test_$(Get-Random)"
    New-Item -ItemType Directory -Force $tmpRun | Out-Null
    Set-Content (Join-Path $tmpRun "serez.json") `
        '{"name":"run-test","version":"1.0.0","scripts":{"hello":"echo hello-from-script"}}' -NoNewline

    Run-CLI-Test "cli: sz run executes script from serez.json" @("run", "hello") `
                 -expectOut "hello-from-script"  -workDir $tmpRun

    Run-CLI-Test "cli: sz run nonexistent script reports error" @("run", "nonexistent") `
                 -expectErr "not found"  -workDir $tmpRun

    Run-CLI-Test "cli: sz run no args reports usage error"  @("run") `
                 -expectErr "Usage: sz run"

    Remove-Item $tmpRun -Recurse -Force -ErrorAction SilentlyContinue

    # local ./packages/ resolution — runs from temp dir so nothing lands in repo root
    $tmpLP = Join-Path $env:TEMP "sz_lp_$(Get-Random)"
    New-Item -ItemType Directory -Force (Join-Path $tmpLP "packages\local-only") | Out-Null
    Set-Content (Join-Path $tmpLP "packages\local-only\index.sz") `
        "fn int localAdd(int a, int b) { return a + b; }`nlet LOCAL_VERSION = `"local-only@1.0.0`";" -NoNewline
    Set-Content (Join-Path $tmpLP "test.sz") `
        "import `"local-only`"; out localAdd(3, 4); out localAdd(-1, 5); out LOCAL_VERSION;" -NoNewline
    $lpLabel = "pkg: import resolves from ./packages/ (not SEREZ_PACKAGES)"
    $lpSkip  = ($filter -and $lpLabel -notlike "*$filter*")
    $lpResult = if ($lpSkip) { $null } else { Invoke-Binary @("`"$(Join-Path $tmpLP 'test.sz')`"") "" $tmpLP }
    if ($lpSkip) {
        # filtered out — run_tests.sh skips it the same way
    } elseif ($lpResult.stdout -match "7" -and $lpResult.stdout -match "4" -and $lpResult.stdout -match "local-only") {
        Write-Host "[PASS] pkg: import resolves from ./packages/ (not SEREZ_PACKAGES)" -ForegroundColor Green
        Add-Result "pass" "pkg: import resolves from ./packages/ (not SEREZ_PACKAGES)"
    } else {
        Write-Host "[FAIL] pkg: import resolves from ./packages/ — stdout: $($lpResult.stdout)" -ForegroundColor Red
        Add-Result "fail" "pkg: import resolves from ./packages/ (not SEREZ_PACKAGES)" "stdout: $($lpResult.stdout)"
    }
    Remove-Item $tmpLP -Recurse -Force -ErrorAction SilentlyContinue
}

# ── Summary ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "TOTAL: $pass passed  $fail failed  $skip skipped" -ForegroundColor $(if ($fail -gt 0) { "Red" } else { "Green" })

if ($tempFile -and (Test-Path $tempFile)) { Remove-Item $tempFile -ErrorAction SilentlyContinue }

# The recorder must agree with the counters. A site that increments without
# recording would silently produce a report missing tests that ran — the same
# class of defect as a suite that cannot find its fixtures.
$recorded = $script:results.Count
$counted  = $pass + $fail + $skip
if ($recorded -ne $counted) {
    Write-Host "Runner defect: $counted outcomes counted but $recorded recorded." -ForegroundColor Red
    exit 1
}

if ($json -ne "") {
    $byCategory = [ordered]@{}
    foreach ($group in $script:results | Group-Object { $_.category }) {
        $byCategory[$group.Name] = [ordered]@{
            passed  = @($group.Group | Where-Object { $_.status -eq "pass" }).Count
            failed  = @($group.Group | Where-Object { $_.status -eq "fail" }).Count
            skipped = @($group.Group | Where-Object { $_.status -eq "skip" }).Count
        }
    }
    $versionLine = (& $binary "--version") 2>&1 | Select-Object -First 1
    # Built by index assignment rather than as one literal: nesting an ordered
    # dictionary or a list inside an `[ordered]@{...}` literal fails with
    # "Argument types do not match" on this PowerShell.
    $report = [ordered]@{}
    $report["schema"]     = "serez-conformance/1"
    $report["runner"]     = "run_tests.ps1"
    $report["platform"]   = "windows"
    $report["core"]       = "$versionLine".Trim()
    $report["startedAt"]  = $startedAt.ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $report["filter"]     = $filter
    $report["totals"]     = [ordered]@{ passed = $pass; failed = $fail; skipped = $skip }
    $report["categories"] = $byCategory
    $report["tests"]      = $script:results.ToArray()
    $report | ConvertTo-Json -Depth 6 | Set-Content -Path $json -Encoding UTF8
    Write-Host "Report written to $json" -ForegroundColor Cyan
}

exit $(if ($fail -gt 0) { 1 } else { 0 })
