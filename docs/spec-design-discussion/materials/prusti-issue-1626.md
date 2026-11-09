Prusti can encode a consuming pure function over a non-`Copy` type, but Rust's move checking prevents specifications from applying that function repeatedly to the same logical value. Could specification-only expressions use unrestricted snapshot values while executable Rust, including executable `#[pure]` function bodies, continues to undergo normal borrow checking?

Extracting specification MIR before borrow checking looks like a way to support this while retaining the existing MIR-based encoder.

## Motivating example: idempotence of a consuming normalization function

Consider an AST that implements neither `Copy`, `Clone`, nor `PartialEq`:

```rust
use prusti_contracts::*;

enum Ast {
    Literal(i32),
    AddZero(i32),
}

#[pure]
fn normalize(ast: Ast) -> Ast {
    match ast {
        Ast::Literal(n) => Ast::Literal(n),
        Ast::AddZero(n) => Ast::Literal(n),
    }
}
```

The implementation consumes its input and returns its normal form. The intended specification is:

```rust
normalize(normalize(ast)) === normalize(ast)
```

Here `===` denotes snapshot equality. Expressing this directly in a specification fails with `E0382`: the first call moves `ast`, so the second call cannot use it. In an inline assertion, consuming a captured AST also encounters the assertion closure's `Fn` capture restriction.

This arises naturally when specifying compiler transformations. A runtime implementation may consume a tree to move its subtrees into the result. Requiring a runtime `Clone` implementation, or changing the normalization interface to take a shared reference, should not be necessary to state its idempotence. A user-defined clone would also need a contract connecting its result to the original logical value.

## Ghost snapshots preserve the value, but do not remove the restriction

Taking a snapshot does not make the original consuming function directly applicable to it:

```rust
let saved = Ghost::new_ref(&ast);

prusti_assert!(
    normalize(normalize(*saved)) === normalize(*saved)
);
```

`Ghost<Ast>` is `Copy`, but its dereference exposes an `Ast` through a shared reference. Passing `*saved` by value fails with `E0507`: cannot move out of the dereference. Copying the wrapper does not change this.

## An equivalent formulation already verifies

The property can be expressed by quantifying two equal inputs and consuming each variable once:

```rust
fn prove_idempotent() {
    prusti_assert!(forall(|a: Ast, b: Ast|
        (a === b) ==> (normalize(normalize(a)) === normalize(b))
    ));
}
```

For a particular saved input, the same technique works:

```rust
fn prove_for_snapshot(input: Ghost<Ast>) {
    prusti_assert!(forall(|a: Ast, b: Ast|
        (a === *input && b === *input)
            ==> (normalize(normalize(a)) === normalize(b))
    ));
}
```

Both formulations verified in a local experiment. Adding `prusti_assert!(false)` after each quantified assertion produced verification errors at both added assertions. The consuming `normalize` implementation was unchanged, and no `Clone`, `trusted`, or `assume` was used.

For this pure function, the two-input formulation is equivalent to ordinary idempotence. The extra variables accommodate Rust's move rules rather than a distinction in the property being verified.

## Existing encoding and a possible implementation route

The current pure/spec encoder already treats MIR `Copy` and `Move` operands identically: both read the place's snapshot. See [`encode_operand_snap`](https://github.com/prusti/prusti/blob/4536912ac5012094ecaed4e01fa8512f6bb47d63/prusti-encoder/src/encoders/mir_pure.rs#L351).

Rustc also separates MIR construction from borrow checking. To check the extraction route, I ran a minimal custom driver on the repository's pinned `nightly-2026-08-15` toolchain. Its `after_expansion` callback queried `tcx.mir_built(def)` and returned `Compilation::Stop`, before whole-crate analysis.

The input contained:

```rust
fn reuse(ast: Ast) {
    let _a = normalize(ast);
    let _b = normalize(ast);
}
```

Ordinary rustc reported `E0382`. The extraction driver exited successfully and obtained MIR containing both moves from the same parameter. The relevant operations, with storage statements omitted, were:

```text
bb0:
    _3 = move _1
    _2 = normalize(move _3) -> bb1

bb1:
    _5 = move _1
    _4 = normalize(move _5) -> bb2
```

Thus obtaining MIR does not require accepting the ownership restrictions imposed by subsequent borrow checking. This experiment exercised early MIR extraction; integrating that route into Prusti's continued verification and compilation is the proposed work.

One possible design is to extract specification-only MIR before borrow checking, preserve its connection to runtime values through snapshots, and erase the specification bodies from the executable compilation path. Executable functions would retain their existing checking and encoding paths. This would let Prusti reuse its MIR lowering and logical encoder without moving specification extraction to THIR solely to avoid move checking.

## Boundaries the design needs to preserve

- **Executable pure functions remain ordinary Rust.** Their implementations must still satisfy Rust's ownership rules. Repeated applications in a specification operate on their logical interpretations.
- **Reading runtime state remains controlled.** A specification may obtain a snapshot only where that read is permitted. It must not consume or mutate runtime state merely because it will be erased.
- **Logical values can be reused.** Once an immutable snapshot is available, using it twice should not require `T: Copy` or a runtime clone.
- **Specifications remain well-defined.** Type checking, initialization checks, pure-call preconditions, and the existing rules for permitted operations still need enforcement. Removing move restrictions must not introduce reads of uninitialized logical locals.
- **Closure integration needs explicit handling.** Rustc organizes borrow checking around type-checking roots, so skipping an individual specification closure's query may not isolate it from its enclosing function. The existing `Fn` capture check also enforces the runtime/specification boundary; its purpose needs to survive a change in extraction strategy.

Would unrestricted reuse of snapshot values be an appropriate semantics for specification-only code? If so, would extracting early MIR and separating specification bodies from executable borrow checking fit the current architecture?

## Related discussions

- [#53: Create a proper notion of equality for Prusti](https://github.com/prusti/prusti/issues/53). [A comment](https://github.com/prusti/prusti/issues/53#issuecomment-673441579) reports that `old(t)` in a specification is rejected because rustc treats it as moving `t`. The issue was later closed with a reference to snapshots and planned improvements to `old`.
- [#1536: Why do pure functions need to return Copy types?](https://github.com/prusti/prusti/issues/1536). This discusses the older restriction on pure function inputs and outputs, including whether passing through a non-Copy value really requires side effects. The present example already accepts the consuming pure function; the remaining obstacle is reuse of its input inside a specification.
- [#1591: Port the user-guide examples off `snap()`](https://github.com/prusti/prusti/pull/1591). The migration uses `*old(&*x)` and borrowed matches to avoid moving non-Copy values in specifications. These are further examples of ownership rules affecting how logical properties must be written.

## Experiment environment

- Prusti checkout: `4536912ac5012094ecaed4e01fa8512f6bb47d63`, with local example files added.
- Rust toolchain: `nightly-2026-08-15`, rustc commit `d453bdd8f092d099bc336f0bda4163f809ad18e0`.
- The quantified examples were run through the Prusti test harness after `./x.py build`.
- The early-MIR extraction experiment used a standalone rustc driver; it did not modify Prusti or rustc.
