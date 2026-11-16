//! Internal runtime/typechecking stubs targeted by `trust2-contract` procedural macros.
//!
//! When the `verify` feature is enabled, macro expansions lower specifications into calls to
//! these helpers so `rustc` typechecks the specification expressions against the surrounding
//! scope. `charon-driver` intercepts and erases them before borrow checking.

/// Marks the start or inline anchor of a specification block.
pub fn entry(_: u32) {}

/// Attaches a precondition closure to the enclosing function.
pub fn precondition<T: FnOnce() -> bool>(_: T) {}

/// Attaches a postcondition closure receiving the return value `T` to the enclosing function.
pub fn postcondition<T, U: FnOnce(T) -> bool>(_: U) {}

/// Trait implemented by `#[invariant(...)]` for types carrying data invariants.
pub trait TypeInvariant {
    /// Returns whether `self` satisfies its type invariant.
    fn invariant(&self) -> bool;
}

/// Asserts a logical predicate at the current program point.
pub fn contract_assert<T: FnOnce() -> bool>(_: T) {}

/// Assumes a logical predicate at the current program point.
pub fn contract_assume<T: FnOnce() -> bool>(_: T) {}

/// Universal quantifier over logical values of type `T`.
pub fn forall<T, U: FnOnce(T) -> bool>(_: U) -> bool {
    true
}

/// Existential quantifier over logical values of type `T`.
pub fn exists<T, U: FnOnce(T) -> bool>(_: U) -> bool {
    true
}

/// Logical implication `p ==> q`.
pub fn implies(_: bool, _: bool) -> bool {
    true
}

/// Evaluates a mutable reference expression in the pre-state of a postcondition.
pub fn old<T>(x: &mut T) -> &mut T {
    x
}
