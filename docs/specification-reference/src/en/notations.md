{{#rule contract.lexical}}

# Lexical Rules

Specifications occur within Rust source code. Function contracts and type invariants use attributes; statement annotations and quantifiers use macro invocations. Comments do not introduce specifications.

## Tokens and identifiers <a id="rust-syntax"></a>

Identifiers, literals, whitespace, and comments follow Rust's lexical rules. Whitespace and ordinary comments may separate tokens wherever Rust permits them. The specification language introduces no separate literal notation or mathematical Unicode operators.

The following names in the grammar refer to Rust syntax:

| Name | Meaning | Examples |
| --- | --- | --- |
| `IDENTIFIER` | Identifier | `x`, `result`, `limit` |
| `Type` | Type | `u8`, `&T`, `Vec<i32>` |
| `Literal` | Literal expression | `true`, `16`, `0u8` |
| `Path` | Expression path | `x`, `u8::MAX` |
| `TUPLE_INDEX` | Tuple-field index | `0` in `pair.0` |

Names introduced by bindings follow Rust scope rules. In a postcondition, `result` is an ordinary parameter name; another identifier may be used instead. In a type invariant, `self` refers to the value whose invariant is being described.

## Grammar notation <a id="notation"></a>

Syntax blocks use a compact extended BNF (EBNF). Boxed monospace text denotes literal source tokens; linked names denote nonterminals.

| Notation | Meaning |
| --- | --- |
| `A := B` | Defines A in terms of B |
| `A B` | A followed by B |
| `A \| B` | Either A or B |
| `A?` | Zero or one occurrence of A |
| `A*` | Zero or more occurrences of A |
| `(A B)` | Groups grammar elements |

A boxed `(`, `)` or `|` is a source token, not a grammar operator. `Arguments?` permits an omitted argument list; `","?` permits a trailing comma. The **Show syntax diagrams** button displays the same production as a diagram.

The grammar uses unqualified names, parenthesized macro invocations, and simple parameter bindings. The `Expr` production describes expression structure; Rust [operator precedence](expressions.md#operators) determines grouping. It is not an unambiguous parser grammar for all Rust expressions.

## Example

```rust,ignore
#[precondition(x /* input */ < 16)]
#[postcondition(|result: u8| result >= x)]
```

The comment separates tokens without changing the precondition. The bars in the second attribute delimit the return-value binding.

## Implementation boundaries

- **Frontend `rust-llbc`**: uses Rust lexical rules, expression parsing, and typing and borrowing checks.
- **Backend `llbc-why3`**: handles extracted representations rather than these lexical forms directly.

### Remaining work

**Distinguishing markers from ordinary names (frontend).** Name-based replacement of `implies` and postcondition `old` also changes ordinary Rust names. For example, `.old()` in `#[postcondition(|r| r == value.old())]` is replaced even when it denotes a user-defined method, causing expansion to fail. Name handling needs to preserve ordinary method calls while recognizing specification markers. See [Name rewriting](constraints.md#name-rewriting).
