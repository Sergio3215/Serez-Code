# -*- coding: utf-8 -*-
"""fuzz_parser.py — fuzzing ligero del parser/lexer de serez-code.

Parte de la jornada de seguridad pre-Fase 1: alimenta al binario `sz` con
entradas basura y mutaciones de un corpus valido, y verifica que NUNCA haya
un panic de Rust (el proceso debe terminar limpio con un error controlado).

Uso:  python fuzz_parser.py [N_CASOS] [SEED]
Sale con codigo 1 si encuentra algun panic/crash; guarda los casos en
fuzz_failures/ para reproducirlos.
"""
import os
import random
import subprocess
import sys
import tempfile

SZ = os.path.join(os.path.dirname(os.path.abspath(__file__)), "target", "release", "sz.exe")
N = int(sys.argv[1]) if len(sys.argv) > 1 else 300
SEED = int(sys.argv[2]) if len(sys.argv) > 2 else 20260702
random.seed(SEED)

CORPUS = [
    'let x = 1; out x;',
    'fn int f(int a) { return a * 2; } out f(21);',
    'let d <string, int> = ({"a", 1}); out d["a"];',
    'let s = new Set([1, 2]); out s.has(1);',
    'let a = [1, 2, 3]; a.push(4); out a.length();',
    'try { let x = 1 / 0; } catch (e) { out e.kind; }',
    'class C { public C() { this.v = 1; } } let c = new C(); out c.v;',
    'let r = [1,2,3].map(x => x * 2); out r;',
    'for (let i = 0; i < 3; i = i + 1) { out i; }',
    'out "interp {1 + 2} fin";',
    'use permissions { Time }',
    'enum E { A, B } out E.A;',
    'let t = 1 == 1 ? "si" : "no"; out t;',
]

TOKENS = [
    "let", "fn", "out", "class", "if", "else", "while", "for", "try", "catch",
    "finally", "throw", "return", "new", "enum", "const", "unsafe", "{", "}",
    "(", ")", "[", "]", ";", ",", ".", "=>", "==", "=", "+", "-", "*", "/",
    "%", "**", "&&", "||", "!", "?", "??", "?.", ":", "<", ">", '"', "'",
    "r\"", "0x", "0b", "1e", "9223372036854775807", "1.5m", "{", "\\", "❤",
    "\x00", "\x01", "￿", "int", "string", "any", "this", "super", "is",
]

def garbage(rng):
    n = rng.randint(1, 120)
    return "".join(rng.choice(TOKENS) if rng.random() < 0.7 else chr(rng.randint(32, 0x2FFF)) for _ in range(n))

def mutate(rng, src):
    s = list(src)
    for _ in range(rng.randint(1, 8)):
        op = rng.random()
        if not s:
            break
        i = rng.randrange(len(s))
        if op < 0.4:
            s[i] = chr(rng.randint(32, 0x2FFF))
        elif op < 0.7:
            s.insert(i, rng.choice(TOKENS))
        else:
            del s[i]
    return "".join(s)

def make_case(rng, i):
    r = rng.random()
    if r < 0.35:
        return garbage(rng)
    if r < 0.85:
        return mutate(rng, rng.choice(CORPUS))
    # anidamiento profundo / entradas largas
    depth = rng.randint(50, 400)
    open_c, close_c = rng.choice([("(", ")"), ("[", "]"), ("{", "}")])
    return "let x = " + open_c * depth + "1" + close_c * depth + ";"

def main():
    if not os.path.exists(SZ):
        print(f"no existe {SZ}; compila primero (cargo build --release)")
        return 2
    faildir = os.path.join(os.path.dirname(SZ), "..", "..", "fuzz_failures")
    tmpdir = tempfile.mkdtemp(prefix="szfuzz")
    failures = 0
    for i in range(N):
        src = make_case(random, i)
        path = os.path.join(tmpdir, f"case_{i}.sz")
        with open(path, "w", encoding="utf-8", errors="replace") as f:
            f.write(src)
        try:
            r = subprocess.run([SZ, path], capture_output=True, timeout=10, text=True, errors="replace")
            crashed = ("panicked at" in (r.stderr or "")) or (r.returncode not in (0, 1))
        except subprocess.TimeoutExpired:
            crashed = True
            r = None
        if crashed:
            failures += 1
            os.makedirs(faildir, exist_ok=True)
            keep = os.path.join(faildir, f"fail_{SEED}_{i}.sz")
            with open(keep, "w", encoding="utf-8", errors="replace") as f:
                f.write(src)
            code = r.returncode if r else "TIMEOUT"
            print(f"[CRASH] caso {i} (exit={code}) guardado en {keep}")
            if r and "panicked at" in r.stderr:
                print("        " + [l for l in r.stderr.splitlines() if "panicked" in l][0][:160])
    print(f"fuzz: {N} casos, {failures} crashes/panics (seed {SEED})")
    return 1 if failures else 0

if __name__ == "__main__":
    sys.exit(main())
