{{#rule contract.specifications}}

# Specification Language

Specifications describe conditions on function calls, program states, and data. They are written as Rust attributes and macro invocations.

{{#syntax Specification}}

[Lexical Rules](notations.md) defines the notation and Rust syntax used throughout this reference. [Logic Expressions](expressions.md) defines the expressions from which specification conditions are formed.

The remaining chapters describe the places where these conditions apply:

- [Function Contracts](function-contracts.md): requirements on calls and guarantees on return.
- [Statement Annotations](statements.md): assertions and assumptions at a point in a function body.
- [Type Invariants](invariants.md): conditions associated with a type.

Each construct is described by its syntax, semantics, examples, and current implementation status. The [Grammar Index](grammar.md) collects all productions.
