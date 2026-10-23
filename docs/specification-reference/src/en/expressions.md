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

The frontend has extraction examples involving slice lengths, Vec indexing, field comparisons, and ordinary calls. Backend support for these expressions varies, as described below.

Conditions can also use Rust expression blocks, local bindings, casts, and branches. For example, this precondition can be extracted:

```rust,ignore
#[precondition({
    let y = x as u32;
    if flag { y > 0 } else { y == 0 }
})]
pub fn selected(x: u8, flag: bool) -> u8 {
    x
}
```

Enum conditions can use `match`, as shown in the [type-invariant examples](invariants.md#r-contract.invariant.example). Ordinary calls can also enter specifications, such as `#[precondition(positive(x))]` with a Rust function `positive` returning `bool`. Extraction does not establish that every operation involved is supported by backend verification.

{{#rule contract.expression.boundary}}

## Implementation boundaries <a id="contract.expression.boundary-title"></a>

- **Frontend `rust-llbc`**: extracts basic operations, field and slice access, ordinary calls, and conditions using `if`, `match`, and casts.
- **Backend `llbc-why3`**: supports basic scalar expressions; slices, indexing, `usize`, and some enum and trait operations still have gaps.

### Remaining work

**Slice conditions (backend).** The example's `i < values.len() && values[i] > 0` can be extracted. Verification still needs translation of slices, lengths, index types, and indexing to express that the index is valid and the element positive.

**Calls within specifications (frontend and backend; rules remain open).** Ordinary calls such as `#[precondition(positive(x))]` can enter LLBC. The frontend also accepts a call to a function that modifies global state before returning `bool`; `Fn + Copy` alone does not exclude such side effects. Which functions are admissible and how their side effects are restricted need to be defined, then enforced through frontend checks and backend logical representations. Extracting an ordinary call is not enough to establish that it can be verified as a logical condition.
