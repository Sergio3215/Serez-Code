# Classes and interfaces

This revision freezes construction-target/shape validation, the audited
constructor/method `super` subset and ordinary member-dispatch validation. Other
initialization and receiver rules are listed under **Coverage boundary** rather
than being implied here.
Normative words such as "must" describe compatibility requirements.

## Construction targets

`new Name(...)` must resolve `Name` to a declared class, a declared interface or
a separately documented built-in construction target such as `Set` or `Tensor`.
An unknown target raises catchable `ReferenceError` (`SZ4001`).

### A reserved namespace name cannot be declared

A `class`, `interface` or `enum` may not be named after one of these seven
runtime namespaces:

`Task` · `Time` · `DateTime` · `System` · `Gui` · `Dec` · `Media`

```serez
// runtime-error-example: the semantic phase rejects the declaration
class Task { }
```

The rejection is a **fatal** `SZ8000` from the semantic phase (`errors.md`), so
the program does not run and `sz` exits `1`. It applies to the declaration only:
`let Task = 1;` is legal, because a variable does not introduce a nominal type.

**The list is seven of the runtime's twenty-two namespaces**, and that is a known
inconsistency rather than a design statement. `class Math { }` is accepted, and a
program may then declare it, call `new Math(...)`, and still call `Math.floor(3.7)`
— two unrelated things of the same name in one program, told apart only by the
shape of the call site. Extending the list to all twenty-two is a breaking change
and needs the process in `compatibility.md`.

Until 10.0.0 this rule was enforced by the parser, which reported it as a syntax
error and abandoned the half-parsed declaration — so a class with a body produced
two invented `Unexpected token '}'` errors alongside the real one. It now runs
after parsing, against a complete declaration, and reports once.

### A declared field type holds for the object's whole life

```serez
class Config {
    timeout: int = 30;
    public Config() { }
}
let c = new Config();
c.timeout = 45;      // fine
```

```serez
// runtime-error-example: the declared type is checked on every write
class Config { timeout: int = 30; public Config() { } }
let c = new Config();
c.timeout = "str";
```

An off-type write is a catchable `TypeError` (`SZ4002`), from inside the class
and outside it, and the field keeps the value it had. Inherited fields count: a
parent's `count: int` constrains a write made through a subclass. Interface
fields are checked on every write too, not only when the instance is built.

The rule is `types.md`'s matching table, not a comparison of type names, so a
`string?` field accepts a string or `null` and a `[int]` field accepts any array.

Two things are deliberately **not** constrained:

- a field declared with a default and no annotation — `bare = 2;` — promises
  nothing and accepts anything;
- a field created by assignment — `c.brandNew = 1;` — remains a documented idiom.
  Enforcing declared types and forbidding undeclared fields are separate
  questions, and only the first has been decided.

Until 10.0.0 the annotation was a default that was never checked again.

### A name may not be declared twice in one scope

Two `class`, two `interface`, two `enum` or two `fn` declarations of the same
name at the same scope are a **fatal** `SZ8000` from the semantic phase.

```serez
// runtime-error-example: the semantic phase rejects the second declaration
class Shape { public Shape() { this.sides = 1; } }
class Shape { public Shape() { this.sides = 2; } }
```

The **second** declaration is reported and the message names the first one's
line. Every collision in a file is reported, not only the first.

Until 10.0.0 the second declaration silently replaced the first and the program
ran with the later definition, which is a hazard in a file long enough that a
reader cannot see both. Serez has no overloading, so two `fn` declarations of one
name are a collision rather than a signature set; nothing inspects parameters.

**The rule is per scope and per kind.** Shadowing between different scopes is
unaffected — a class declared inside a function body does not collide with one
outside it. A `class` and an `interface` of the same name are *not* reported;
that is the separate hazard described immediately below, and changing it is a
language decision that has not been taken.

**Two files are not one scope.** A module whose `import` redeclares a name the
importing file holds is a different rule with its own reporting; see
`modules.md`.

### An unresolvable parent class is rejected before the program runs

A class declaring a parent that cannot be resolved is a **fatal** `SZ8000` from
the semantic phase.

```serez
// runtime-error-example: the semantic phase rejects the declaration
class Child : Missing { public Child() { } }
out 1;
```

Until 10.0.0 this program printed `1` and exited `0`. The parent was resolved
when an instance was built, so a class that was never constructed was never
checked, and `--check` could not tell you that you inherited from something that
does not exist. Constructing one has always been a catchable `ReferenceError`
(`SZ4001`), and still is when the declaration itself cannot be judged.

**"Not declared in this file" is not "does not exist", and the rule respects
that.** It reports only what it can prove:

| Case | Reported |
| --- | --- |
| Parent declared anywhere at the top level, before or after the child | no |
| Parent declared in an enclosing scope | no |
| A built-in construction target — `Error`, `Set`, `Tensor` | no |
| **The file contains any `import`** | **no**, for any parent |
| Parent declared nowhere, in a file that imports nothing | **yes** |

A file with an `import` is never reported against, because the phase resolves one
file at a time and an imported module may legitimately declare the parent. An
`import` that in fact fails to supply the name is therefore *not* caught: proving
that requires resolving and parsing modules during the semantic phase, which
would read the filesystem at check time and has consequences under `--eval`
lockdown. That is recorded as an open decision rather than taken.

**The rule applies to classes declared at the top level.** Inside a body, a class
may legitimately be written before the parent it names — the class registry is
global and a forward reference resolves once the later declaration has run — so
the phase does not judge those.

### A class and an interface cannot share a name

They live in separate registries, so both declarations are accepted — and
`new Name(...)` consults the **interface**, regardless of which was declared
last. The class becomes unreachable:

```serez
// runtime-error-example: an interface is not constructed with new
class Shape { public Shape() { this.tag = "class"; } }
interface Shape { tag: int; }

new Shape();              // TypeError / SZ4002 — the interface wants field form
new Shape({ tag: 1 });    // builds the interface; the class is dead code
```

The failure appears at the construction site as an argument-form error, with
nothing pointing back at the declaration that shadowed it, so the evaluator now
warns at the second declaration. It is a warning rather than a refusal because
refusing would be a breaking change; no official package has such a collision.

Only this pair collides. An `enum` may share a name with a class or an interface
harmlessly, because `new Name(...)` and `Name.Variant` are different syntax
reaching different registries. Both are pinned in `tests/runtime_outcome.rs`.

Redeclaring the *same* kind twice is ordinary shadowing: the later declaration
wins, as it does for a `let`.

## Interface instances

An interface is constructed with exactly one field-form argument:

```serez
interface Point {
    x: int;
    y: int;
}

let point = new Point({ x: 3, y: 4 });
```

Every declared field must be supplied, no undeclared field may be supplied, and
each value must match its declared type. Positional arguments, missing fields,
extra fields and field type mismatches raise catchable `TypeError` (`SZ4002`).

This is exact construction, not the later partial object-patch operation.

## Class instances

A concrete class is constructed with positional arguments:

```serez
class Point {
    public Point(int x, int y) {
        this.x = x;
        this.y = y;
    }
}

let point = new Point(3, 4);
```

- A field-form argument is invalid for a class and raises catchable `TypeError`
  (`SZ4002`).
- An abstract class cannot be instantiated directly; attempting it raises the
  same catchable `TypeError`.
- A class with no constructor accepts no arguments. Supplying any raises the
  same catchable `TypeError`.
- A declared constructor follows the positional arity/default/rest contract in
  `functions.md`. An invalid argument count raises the same catchable
  `TypeError` before the constructor body runs.

The human-readable messages remain actionable but are not stable identifiers;
tooling and programs must classify on `code` and `kind`.

## Inheritance graph

A class has at most one named parent. Parent names may be forward references, so
`class Child : Parent {}` may appear before `Parent` is declared. The hierarchy
cannot be used while that reference is unresolved: constructing `Child`, an
implicit/explicit `super()` call, or parent dispatch raises catchable
`ReferenceError` (`SZ4001`) identifying the missing parent. Declaring the parent
later makes the existing child hierarchy usable.

The complete class graph must be acyclic. A declaration that would introduce
self-inheritance or an indirect cycle is rejected atomically with catchable
`TypeError` (`SZ4002`); the rejected class is not inserted. Method, getter and
setter ancestor walks are additionally bounded by the number of registered
classes, so even a corrupt legacy/internal registry cannot make lookup loop
forever.

A `sealed` class may be instantiated but cannot be used as a parent. Attempting
to extend it is also catchable `TypeError` (`SZ4002`). These rules validate graph
shape; abstract-method completeness and declaring-owner privacy remain separate
contracts/debts below.

## Constructor chaining

`super(args...)` is valid only while a class constructor is running. It resolves
the direct parent's constructor and runs it against the same `this` instance.
Its positional arguments, defaults and final rest parameter follow
`functions.md`.

The compatibility rules for an omitted explicit call are:

- If the parent constructor has no required parameters, it is called
  implicitly before the child constructor body.
- If the child declares no constructor, the same implicit call occurs. A parent
  constructor with required parameters cannot be supplied implicitly and raises
  catchable `TypeError` (`SZ4002`) at `new Child()`.
- If the child has its own constructor and the parent requires arguments, an
  omitted `super(...)` remains allowed for compatibility. No parent constructor
  runs; the child may initialize inherited fields itself.
- Detection is currently a conservative syntactic scan. Any `super(...)`
  occurrence anywhere in the child constructor, including only one branch,
  suppresses implicit chaining. This is a compatibility behavior and a known
  semantic risk, not a recommendation for conditional constructor calls.

An empty `super()` against a parent with no declared constructor is a compatible
no-op. Supplying arguments in that case is invalid instead of silently ignoring
them and raises catchable `TypeError` (`SZ4002`). Calling `super()` outside a
constructor, on a class without a parent, or with invalid arity raises the same
error.

### Implicit chaining reaches exactly one level

Running one parent's constructor does not synthesize a call to the next
ancestor, and the implicit rule above applies **only at the outermost `new`**.
A constructor invoked *as a parent* — reached by an explicit `super()` or by the
implicit call — does not get an implicit call of its own.

The chain therefore continues past the first level only through an explicit
`super(...)` in each intermediate constructor:

```serez
// runtime-error-example: the first half shows the chain stopping
class G   { public G()   { this.a = "G"; } }

class MidNo  : G      { public MidNo()  { this.b = "b"; } }          // no super()
class LeafNo : MidNo  { public LeafNo() { this.c = "c"; } }
new LeafNo().a;          // ReferenceError — G's constructor never ran

class MidYes  : G       { public MidYes()  { super(); this.b = "b"; } }
class LeafYes : MidYes  { public LeafYes() { this.c = "c"; } }
new LeafYes().a;         // "G" — the explicit super() carried the chain up
```

A class with **no constructor at all** in the middle stops the chain the same
way: `class MidNone : G { }` leaves `G`'s constructor unrun when `MidNone` is
reached as a parent, even though `new MidNone()` on its own would run it.

The failure mode is a field that was never initialized, so it surfaces wherever
that field is first read rather than at construction. In a hierarchy deeper than
two levels, write `super(...)` in every intermediate constructor and do not rely
on the implicit rule.

## Parent-method dispatch

`super.method(args...)` is valid inside a class method. Resolution begins at the
direct parent and walks upward, deliberately bypassing an override on the
current class. The selected method receives the current instance as `this` and
uses the parameter/default/rest contract from `functions.md`.

Calling it outside a class method, from a class without a parent, or with invalid
arity raises catchable `TypeError` (`SZ4002`). A parent chain with no matching
method raises catchable `ReferenceError` (`SZ4001`).

## Ordinary member dispatch

An instance method call begins lookup on the instance's runtime class and walks
its single-inheritance chain upward. Calling a resolved instance or static method
uses the positional/default/rest contract from `functions.md`; an invalid count
raises catchable `TypeError` (`SZ4002`) after argument evaluation and before the
method body.

For a dot expression without parentheses, instance lookup prefers an existing
field, then a getter, then a bound method reference. Calling an existing field
as a method remains allowed only when that field contains a callable value. The
universal `toString()` fallback is unchanged. If none of those routes resolve,
the access raises catchable `ReferenceError` (`SZ4001`).

A private method is private **to the class that declares it**. Calling it, or
taking its bound reference, is allowed only from inside that class's own methods;
everywhere else it raises catchable `TypeError` (`SZ4002`), and catchability does
not grant access. If a method returns a value incompatible with its declared
return type, the completed call raises the same error after its call scope is
unwound.

### What `private` means, exactly

| Access | Allowed |
| --- | --- |
| A method of `Base` uses `Base`'s private member | yes |
| ...including when the receiver is a `Derived` instance | yes |
| A method of `Derived` uses `Base`'s private member | **no** |
| An unrelated class uses `Base`'s private member | no |
| Code outside any class uses `Base`'s private member | no |

**Inheritance does not widen `private`.** A subclass may call an accessible
parent method that itself uses the private member — that is the parent's own
access, and it keeps working. What the subclass may not do is reach the member
directly.

The check is keyed to the declaring class, and so is the execution context: a
method body runs as the class that declared it, which is what makes row two of
the table true. Until 10.0.0 both were keyed to the *receiver's runtime class*,
so a subclass reached an inherited private and `private` in practice meant "not
reachable from outside the hierarchy".

`ClassName.method(...)` requires a static method declared for the named class.
A missing static method raises catchable `ReferenceError` (`SZ4001`) identifying
the class/member, rather than reporting the class name as an undeclared variable.
Static inheritance and static method references remain outside this audited
contract.

## Fields and computed properties

For `obj.name` without parentheses, an existing stored field takes precedence
over a getter of the same name. Otherwise a matching getter is invoked with no
implicit arguments. `obj.name = value` invokes a matching setter with the
assigned value; if no accessor exists, the stored field is updated or created.

A **declared** class field wins over a getter of the same name; that is the only
way the two can coexist, because the getter-only check fires on any write:

```serez
class D {
    stored: string = "declared-field";      // read wins over the getter
    public D() { }
    public get string stored() { return "getter"; }
}
```

The same check makes a subclass getter break an inherited constructor. A parent
that stores `this.v` and a child that declares `get v()` cannot be constructed:
the parent's own `this.v = …` is refused as a write to a getter-only property,
raising `TypeError` (`SZ4002`) from `new Child()`. Naming a getter after a field
the parent assigns is therefore a breaking change to the parent.

Getter/setter lookup walks the same inheritance chain as methods. External use
of a private accessor, malformed getter/setter arity and an incompatible declared
return value raise catchable `TypeError` (`SZ4002`). A property with a getter but
no setter is read-only: assignment remains refused with the same error. Assigning
a field on a non-instance also raises `SZ4002`. A user `throw` or structured
runtime error from an accessor body propagates unchanged.

These errors do not make setter execution transactional. Mutations performed by
a setter before it fails remain ordinary program mutations; the runtime only
guarantees that rejecting the write before entering a setter does not perform a
raw field fallback.

### Known property compatibility debt (non-normative)

The following observed behaviors are not frozen as desired semantics:

- Interface construction is exact, and a later direct assignment can still add
  an *undeclared* field. Replacing a *declared* field with a value of another
  type is refused; see "A declared field type holds for the object's whole life".
- Simple and nested field assignment evaluate the right-hand side before fully
  validating the receiver/path.

Changing any of these can break dynamic object patterns or evaluation order. It
requires owner-aware member metadata/property schemas, dedicated migration tests
and an ecosystem review; it must not be folded into diagnostic cleanup.

## Conformance evidence

- `tests/runtime_outcome.rs`: all nine target/interface/class validation paths,
  all nine `super` validation paths, stable codes, catchability, cleanup and
  evaluator reuse without stale payloads.
- `tests/unit_catchable_core.sz`: language-level `try/catch` coverage for the
  same validation matrix.
- `tests/err_undeclared_class.sz`, `tests/sec_undeclared_class.sz`,
  `tests/sec_abstract_instantiate.sz` and `tests/err_extra_iface_field.sz`: CLI
  error paths retained for compatibility.
- `tests/09_interfaces.sz`, `tests/08_classes.sz` and the official `serez-ui`
  canary: successful construction remains compatible.
- `tests/unit_super_errors.sz` and `tests/err_super_outside.sz`: language/CLI
  failure matrix and cleanup after caught failures.
- `tests/unit_implicit_super.sz`, `tests/unit_super_method.sz` and
  `tests/unit_bug_b64_b74.sz`: successful implicit, explicit, multi-level,
  default and rest behavior.
- `tests/unit_member_dispatch_errors.sz` and `tests/err_member_missing.sz`:
  instance/static validation, visibility refusal, return checking and cleanup
  after caught failures.
- `tests/unit_property_dispatch_errors.sz`, `tests/err_field_non_instance.sz`
  and `tests/sec_getter_no_setter.sz`: property validation, accessor propagation
  and preservation of receiver state across rejected writes.
- `tests/unit_inheritance_errors.sz`, `tests/err_inheritance_cycle.sz`,
  `tests/err_parent_missing.sz` and the cyclic-registry Rust unit test: graph
  validity, forward-reference recovery, bounded lookup and structured errors.

## Coverage boundary

Class field-default timing, the compatibility debts above, constructor return
behavior, abstract-method requirements, static inheritance/references, static
`super`, closure capture and the complete receiver-writeback surface still
require dedicated implementation and ecosystem audits. The evaluator state
"executing class but no `this` binding" is treated as an internal invariant and
is not exposed as a catchable source-level contract. Existing conformance tests
remain authoritative for uncovered behaviors.
