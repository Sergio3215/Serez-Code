# Values, assignment and mutation

Normative contract for what a value is, what assignment does, and when a
mutation is visible somewhere else.

Every rule here was derived by probing the running implementation. Where the
implementation was inconsistent, that is stated rather than smoothed over.
Every claim below was re-probed against the binary in the current cycle: the
copy rules for arrays, dicts, sets and instances; copying across a call and out
of a container; writeback for all thirteen named mutators and for a method that
assigns to `this`; a getter not being a place; closure capture; the five
equality rows; the eight truthiness rows; and the operand-returning `&&`/`||`.
One example did not survive that pass and is corrected below.

## Everything is a value

Assignment copies. Passing an argument copies. Reading out of a container
copies. This holds for every type, including the ones other languages treat as
references:

```serez
let a = [1, 2];
let b = a;      b.push(3);        // a is still [1, 2]

let d <string, int> = ({"k", 1});
let e = d;      e.Add({"j", 2});  // d still has one entry

let s = new Set([1]);
let t = s;      t.add(2);         // s still has one element

class Box { public Box(int v) { this.v = v; } }
let p = new Box(1);
let q = p;      q.v = 99;         // p.v is still 1
```

The same applies across a call:

```serez
fn void mutate([int] xs) { xs.push(9); }
let xs = [1];
mutate(xs);                       // xs is still [1]
```

And to a read out of a container:

```serez
let outer = [[1]];
let inner = outer[0];
inner.push(2);                    // outer is still [[1]]
```

There is no aliasing, no reference type and no way to obtain one. `&x` produces
a pointer value usable only through `*` inside `unsafe { }`; see `security.md`.

## Receiver writeback

The one thing that is *not* a copy is calling a mutating method **on a place**.
When the receiver of a mutator is something you could assign to, the mutation is
written back to that place:

```serez
class H { public H() { this.items = [1]; } }
let h = new H();
h.items.push(2);                  // h.items is [1, 2]

let d <string, any> = ({"list", [1]});
d["list"].push(2);                // d["list"] is [1, 2]

let a = [[1]];
a[0].push(2);                     // a[0] is [1, 2]

class KV {
    public KV() {
        // A dict literal only exists in an annotated `let` — see `dicts.md`.
        let seed <string, any> = ({"k", []});
        this.c = [[seed]];
    }
}
let kv = new KV();
kv.c[0][0]["k"].push(1);          // writes through the whole chain
```

The example above used to read `this.c = [[ ({"k", []}) ]];` with `kv` never
declared. It could not run: a dict literal is not an expression and cannot
appear in a field initialiser.

A **place** is a variable, a field read (`.name`, no parentheses, no
arguments), an index (`[key]`), or any chain of those. The write happens after
the method returns and stores the mutated receiver back through the same path.

The distinction that matters:

| Expression | Effect |
| --- | --- |
| `a[0].push(2)` | Mutates `a[0]`. The receiver is a place. |
| `let x = a[0]; x.push(2)` | Mutates `x` only. The read already copied. |

Writeback applies to the built-in collection mutators — `push`, `pop`, `shift`,
`unshift`, `sort`, `reverse`, `remove`, `add`, `delete`, `Add`, `Remove`,
`RemoveAll`, `clear` — and to a class method that assigns to `this`. A method
that only reads does not pay for a writeback.

If the path does not exist when the write happens (an index that moved out of
range, a field that turned out to be a getter), nothing is written. That is the
behavior that existed before writeback, kept deliberately.

## Closures capture the variable, not its value

A closure shares a cell with the enclosing binding. It sees later writes, and
its own writes persist across calls:

```serez
fn any counter() {
    let n = 0;
    return () => { n = n + 1; return n; };
}
let c = counter();
c(); c(); c();                    // 1, 2, 3

let captured = 10;
let read = () => { return captured; };
captured = 20;
read();                           // 20, not 10
```

This is the one place a binding is shared rather than copied.

## Equality

`==` compares scalars by value and containers by identity:

| Comparison | Result |
| --- | --- |
| `"a" == "a"` | `true` |
| `1 == 1.0` | `true` — int and decimal compare numerically |
| `null == null` | `true` |
| `[1, 2] == [1, 2]` | **`false`** |
| `new Box(1) == new Box(1)` | **`false`** |

Two structurally identical arrays are not equal, and since assignment copies,
no two array values are ever equal — including an array and its own copy. Set
membership and `indexOf` do **not** use `==`: they use a fingerprint that
compares scalars by value and never matches a compound. See `sets.md`.

This is a documented inconsistency, not a design statement. Changing `==` to
compare containers structurally is a breaking change and needs the process in
`compatibility.md`.

## Truthiness

| Value | Truthy? |
| --- | --- |
| `false`, `null` | no |
| `0`, `0.0` | no |
| `""` | no |
| `[]` | no |
| everything else, including `[0]` and `"0"` | yes |

`&&` and `||` return one of their operands, not a boolean: `a && b` yields `a`
when `a` is falsy and `b` otherwise; `a || b` yields `a` when `a` is truthy and
`b` otherwise. See `operators.md`.

## Limits

Copying a value bounds its own recursion at 500 levels of nesting. A value
deeper than that cannot be copied and the program stops with a fatal
`ResourceError` (`SZ6002`) rather than silently losing everything below that
depth. See `limits.md`.
