# Conformance

How a rule in this specification is identified, and how the implementation is
shown to satisfy it.

## The problem this solves

Before M8, `spec/` and `tests/` were both good and unconnected. A specification
document stated a rule in prose; a test asserted a behaviour; and whether the two
were about the same thing was something a reader worked out. Nothing could answer
*"which test proves this sentence?"* or, more importantly, *"which sentences does
nothing prove?"*

Identifiers close that gap in the only way that survives contact with a growing
codebase: mechanically.

## Identifiers

A **normative rule** — a statement of behaviour the implementation must satisfy —
carries an identifier at its definition site:

```markdown
**[MEM-002]** A freed handle's value is **never reissued**.
```

The prefix names the area, the number is sequential within it and is **never
reused**. A rule that is deleted takes its number out of circulation with it, so a
reference in an old commit, an issue or a changelog still resolves to the rule it
meant.

Prefixes are three or four uppercase letters. `MEM` for memory, `ARR` for arrays,
and so on; a new area introduces its prefix in its own document.

## What counts as a rule

Not every sentence. A rule is a statement that could be **contradicted by an
implementation** — a value, an error, an ordering, a limit, a guarantee.

Prose that explains *why*, describes a use case, or points at another document is
not a rule and gets no identifier. Numbering everything would produce a document
where the identifiers are noise and the coverage figure is meaningless.

## How a rule is proved

A test declares which rules it covers, with a marker:

```serez
// conformance: MEM-002
unsafe {
    let a = Memory.alloc(8);
    Memory.free(a);
    let b = Memory.alloc(8);
    assert((a == b) == false, "the freed id did not come back");
    Memory.free(b);
}
```

```rust
//! conformance: MEM-012
```

The marker is the whole mechanism. It works in both languages, needs no build
step, and puts the claim next to the assertion rather than in a table someone has
to remember to update.

## The checker

`tests/conformance_map.rs` enforces three properties:

1. **Every identifier is defined exactly once.** A duplicate is a numbering
   mistake and is caught at the moment it is made.
2. **Every identifier a test references exists.** A test claiming to cover a rule
   that was renamed or deleted is a stale claim, and a stale claim is worse than
   no claim.
3. **Every identifier has at least one test.** This is the property that makes the
   scheme worth having: it is impossible to add a rule to `spec/` and forget to
   prove it, because the build fails until it is proved.

Property 3 is why identifiers are added **as an area is covered**, not all at
once. An identifier is a commitment that something verifies the rule; assigning
one to a rule nothing tests would only record the gap in a new place.

## Coverage today

`MEM-001` to `MEM-015` — `spec/memory.md`, the first area covered, and the worked
example for everything above. Its rules are proved by
`tests/unit_memory_conformance.sz`.

Every other `spec/` document is prose without identifiers. That is the honest
state: 30 documents, one covered. `docs/maturity/ROADMAP_STATE.md` §9N records
the order the rest are planned in and why memory went first.

## Adding an area

1. Read the implementation and **probe it**. A rule derived from source-reading
   alone is a guess; the specification's own preamble says every rule in it was
   derived by probing, and that is the standard.
2. Write the rules, with identifiers, at their definition sites.
3. Write or **identify existing** tests that prove them. Reuse is preferred:
   several areas are already well tested, and duplicating an assertion to attach
   a marker to it adds maintenance without adding evidence.
4. Run `cargo test --test conformance_map`. It fails until every new identifier
   is covered.
