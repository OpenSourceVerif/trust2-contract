{{#rule contract.postcondition}}

# Postconditions

{{#syntax Postcondition ResultPredicate}}

## Semantics

`#[postcondition(|result| Q)]` requires the Boolean condition `Q` to hold on normal return, with `result` bound to the returned value. Verification of the function must establish this condition under its preconditions.

Multiple postconditions state simultaneous guarantees. They do not specify whether the function terminates or what happens on a panic.

## Return-value binding <a id="result-binding"></a>

The parameter name is arbitrary: `result` is not a keyword. Its type is the function's return type and may be written explicitly. A function without a declared return type returns `()`.

Function parameters and method receivers are referenced from the surrounding scope. A captured reference does not by itself denote a snapshot of the data it points to. The intended pre-state notation and its implementation limits are described under [Old-state Expressions](old.md).

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[postcondition(|value: u32| value == x)]
pub fn identity(x: u32) -> u32 {
    x
}
```

Here `value` denotes the returned value, and `x` is the input used in the comparison.

## Implementation status

The postcondition must satisfy `Fn(ReturnType) -> bool + Copy`. The macro supplies the function's return type to the internal interface; Rust checks the closure's parameter, result type, and captures.

The frontend extracts postconditions and associates them with their functions. Existing integration examples cover this path. The grammar shows inline closures: parsing an expression does not establish support for arbitrary function pointers or callable objects.

Name rewriting for `old` is enabled only in postconditions. This does not provide general snapshots or relax the closure's borrowing constraints.
