# ── Serez-Code Test Runner ────────────────────────────────────────────────────
# Usage:
#   .\run_tests.ps1                    # run all tests
#   .\run_tests.ps1 -filter "switch"   # run tests whose name contains "switch"
#   .\run_tests.ps1 -generate          # regenerate .expected golden files
#   .\run_tests.ps1 -unit              # only run unit_*.sz tests (using framework)
#   .\run_tests.ps1 -e2e               # only run E2E tests (numbered NN_*.sz)
#
# ── Test types ────────────────────────────────────────────────────────────────
#
#   tests/NN_*.sz        → E2E tests
#                          Run the file and compare stdout vs tests/NN_*.expected
#                          Use -generate to create/update the .expected file.
#
#   tests/unit_*.sz      → Unit tests
#                          The framework (tests/framework.sz) is prepended.
#                          Each file calls test("name", () => { assert(...) })
#                          and summary() at the end.
#                          PASS = no [FAIL] line in stdout.
#
#   tests/err_*.sz       → Error tests
#                          The program must emit at least one ❌ line on stderr.
#                          PASS = at least one ❌ found.
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
#   unit_closures_mutable  Closures con estado mutable: make_counter (1→2→3), dos contadores
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
# ── Error tests (tests/err_*.sz) ──────────────────────────────────────────────
#
#   err_arity              Llamar función con número incorrecto de argumentos
#   err_bang_nonbool       Operador ! sobre valor no booleano
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
#   err_private            Acceso a campo privado de clase desde fuera
#   err_return_toplevel    return en nivel superior (fuera de función)
#   err_return_type_mismatch Función declarada int devuelve string
#   err_sort_mixed         sort sobre array con tipos mezclados (int y string)
#   err_type_param         Parámetro tipado recibe tipo incorrecto
#   err_typed_push         push de tipo incorrecto en array tipado [int]
#   err_undeclared         Leer variable no declarada
#   err_undeclared_assign  Asignar variable no declarada (sin let)
#   err_undeclared_class   new de clase no definida
#
# Exit code: 0 = all passed, 1 = failures found
# ─────────────────────────────────────────────────────────────────────────────

param(
    [string]$filter    = "",
    [switch]$generate  = $false,
    [switch]$unit      = $false,
    [switch]$e2e       = $false
)

$ErrorActionPreference = "Stop"
$root       = $PSScriptRoot
$testsDir   = Join-Path $root "tests"
$framework  = Join-Path $testsDir "framework.sz"
$binary     = Join-Path $root "target\debug\sz.exe"
$tempFile   = Join-Path $env:TEMP "sz_test_temp.sz"

# ── Build first ───────────────────────────────────────────────────────────────
Write-Host "Building..." -ForegroundColor Cyan
Push-Location $root
$buildOut = cargo build 2>&1
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

function Invoke-Sz([string]$runFile) {
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    Start-Process -FilePath $binary -ArgumentList "`"$runFile`"" `
        -NoNewWindow -Wait `
        -RedirectStandardOutput $outFile `
        -RedirectStandardError  $errFile
    $stdout = if (Test-Path $outFile) { Get-Content $outFile } else { @() }
    $stderr = if (Test-Path $errFile) { Get-Content $errFile } else { @() }
    Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue
    return @{ stdout = $stdout; stderr = $stderr }
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
        # Error tests: must have at least one ❌ on stderr
        $hasError = ($stderr | Where-Object { $_ -match "^❌" }).Count -gt 0
        if ($hasError) {
            Write-Host "[PASS] $label" -ForegroundColor Green
            $script:pass++
        } else {
            Write-Host "[FAIL] $label — expected an error but got none" -ForegroundColor Red
            $script:fail++
        }
        return
    }

    if ($isUnit) {
        # Unit tests: look for [FAIL] lines in stdout
        $failures = $stdout | Where-Object { $_ -match "^\[FAIL\]" }
        $summary  = $stdout | Where-Object { $_ -match "^Results:" }
        if ($failures.Count -eq 0) {
            Write-Host "[PASS] $label" -ForegroundColor Green
            if ($summary) { Write-Host "       $summary" -ForegroundColor Gray }
            $script:pass++
        } else {
            Write-Host "[FAIL] $label" -ForegroundColor Red
            $failures | ForEach-Object { Write-Host "       $_" -ForegroundColor Yellow }
            $script:fail++
        }
        return
    }

    # E2E golden file test
    if ($generate) {
        $stdout | Set-Content $expectedFile
        Write-Host "[GEN]  $label → $expectedFile" -ForegroundColor Cyan
        return
    }

    if (-not (Test-Path $expectedFile)) {
        Write-Host "[SKIP] $label (no .expected file — run with -generate to create)" -ForegroundColor Yellow
        $script:skip++
        return
    }

    $expected = Get-Content $expectedFile
    $actual   = $stdout

    if ($null -eq $actual) { $actual = @() }
    if ($null -eq $expected) { $expected = @() }

    $diff = Compare-Object $expected $actual
    if ($null -eq $diff) {
        Write-Host "[PASS] $label" -ForegroundColor Green
        $script:pass++
    } else {
        Write-Host "[FAIL] $label" -ForegroundColor Red
        $diff | ForEach-Object {
            $arrow = if ($_.SideIndicator -eq "<=") { "expected:" } else { "  actual:" }
            Write-Host "       $arrow $($_.InputObject)" -ForegroundColor Yellow
        }
        $script:fail++
    }
}

# ── Discover and run tests ────────────────────────────────────────────────────
$runAll  = -not $unit -and -not $e2e

Write-Host "═══ E2E Tests ════════════════════════════════" -ForegroundColor Cyan
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
if ($runAll -or $unit) {
    Get-ChildItem $testsDir -Filter "unit_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $true $false
    }
}

Write-Host ""
Write-Host "═══ Error Tests ══════════════════════════════" -ForegroundColor Cyan
if ($runAll -or $e2e) {
    Get-ChildItem $testsDir -Filter "err_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $false $true
    }
}

# ── Summary ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "TOTAL: $pass passed  $fail failed  $skip skipped" -ForegroundColor $(if ($fail -gt 0) { "Red" } else { "Green" })

if ($tempFile -and (Test-Path $tempFile)) { Remove-Item $tempFile -ErrorAction SilentlyContinue }

exit $(if ($fail -gt 0) { 1 } else { 0 })
