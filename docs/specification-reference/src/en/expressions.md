{{#rule contract.expression}}

# Logic Expressions

Logic expressions form the conditions used in contracts, statement annotations, and type invariants. They use Rust expressions together with quantifiers, implication, and an old-state marker.

{{#syntax Condition Expr Arguments Field UnaryOp BinaryOp}}

{{#rule contract.expression.intro}}

## Types and values

A `Condition` must have type `bool`. Subexpressions may have other Rust types: for example, in `x + 1 <= limit`, the addition produces a numeric value and the comparison produces a Boolean value. There is no separate predicate type or implicit conversion from integers to Boolean conditions.

Names refer to bindings in scope. Field access, dereferencing, indexing, and function or method calls use Rust typing and borrowing rules. Parentheses group expressions without changing their value.

Integer expressions use Rust integer types, including their finite ranges and arithmetic checks. The specification interface does not introduce mathematical integers or real numbers. An expression that indexes a collection must also satisfy the index bounds; appearing in a specification does not remove that requirement.

## Operator precedence <a id="operators"></a>

The following groups are ordered from highest to lowest precedence. Unary operators associate to the right. Arithmetic and bitwise binary operators associate to the left. Comparisons cannot be chained directly: write `a < b && b < c`.

| Group | Operators |
| --- | --- |
| Access and calls | `e.field` `e[i]` `f(x)` `e.method(x)` |
| Unary | `!` `-` `*` `&` `&mut` |
| Multiplicative | `*` `/` `%` |
| Additive | `+` `-` |
| Shifts | `<<` `>>` |
| Bitwise AND | `&` |
| Bitwise XOR | `^` |
| Bitwise OR | `\|` |
| Comparisons | `==` `!=` `<` `<=` `>` `>=` |
| Logical AND | `&&` |
| Logical OR | `\|\|` |

For Boolean operands, `!`, `&&`, and `||` express negation, conjunction, and disjunction. `&&` and `||` short-circuit: their right operand is evaluated only when needed. The single-character bitwise operators do not provide this short-circuit behavior.

[Quantifiers](quantifiers.md) bind a value of a Rust type. [Implication](implication.md) expresses a conditional proposition. [Old-state Expressions](old.md) describe the pre-state marker available in postconditions.

{{#rule contract.expression.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(i < values.len() && values[i] > 0)]
pub fn positive_at(values: &[u8], i: usize) -> u8 {
    values[i]
}
```

The condition requires a valid index and a positive element. Short-circuit evaluation ensures the second operand is evaluated only when the index is in bounds.

{{#rule contract.expression.boundary}}

## Implementation status <a id="contract.expression.boundary-title"></a>

Existing frontend examples cover field comparisons, ordinary calls, slice lengths, Vec indexing, and trait comparisons, including combinations with quantifiers. An operator's presence in the grammar does not imply coverage for every operand type or overload.

Calls can be retained in specification bodies, but external functions may be opaque. There is no dedicated declaration mechanism for pure logical functions, predicates, or ghost code, and no separate logical sequence, set, or map interface or complete set of standard-library contracts.

Blocks, `if`, `match`, casts, complex patterns, and other Rust forms are not expanded in this core grammar. Their acceptance depends on parsing, Rust typing, and extraction for the particular expression.
