{{#rule contract.function}}

# Function Contracts

A function contract specifies conditions required when a function is called and conditions guaranteed when it returns normally.

{{#syntax FunctionContract}}

{{#rule contract.function.intro}}

## Semantics

<a id="grammar-Precondition"></a>[Preconditions](preconditions.md) describe the caller's obligations and the assumptions under which the function body is verified. <a id="grammar-Postcondition"></a>[Postconditions](postconditions.md) describe the function's obligations on return.

Attributes precede the function definition. Preconditions and postconditions may be repeated and interleaved. All stated preconditions are required; all stated postconditions are guarantees to establish. A postcondition alone does not assert termination.

<a id="grammar-ResultPredicate"></a><a id="result-binding"></a>A postcondition binds the returned value with a closure parameter. See [Return-value binding](postconditions.md#result-binding). References to an earlier state use [Old-state Expressions](old.md), whose current interface is restricted.

{{#rule contract.function.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(x < 16)]
#[postcondition(|result| result >= x)]
pub fn square(x: u8) -> u8 {
    x * x
}
```

The precondition restricts `x` to values whose square fits in `u8`. The postcondition requires the returned value to be at least `x`.

This example supports both frontend extraction and backend verification. Preconditions and postconditions can also be used separately, or repeated on the same function.

Conditions on methods can refer to `self`. The following contract can be extracted and translated to WhyML:

```rust,ignore
pub struct Counter {
    value: u32,
}

impl Counter {
    #[precondition(self.value < 10)]
    #[postcondition(|r| r == self.value)]
    pub fn get(&self) -> u32 {
        self.value
    }
}
```

{{#rule contract.function.boundary}}

## Implementation boundaries <a id="contract.function.boundary-title"></a>

- **Frontend `rust-llbc`**: supports preconditions/postconditions on ordinary functions and methods with bodies.
- **Backend `llbc-why3`**: supports basic scalar-function contract verification; generic functions and trait declarations still have translation limitations.

### Remaining work

**Bodyless declarations (frontend).** Macros currently carry conditions through the function body, so this form cannot expand:

```rust,ignore
trait Reader {
    #[postcondition(|result| result <= 100)]
    fn read(&self) -> u8;
}
```

Conditions need to be stored on the method declaration for later handling of implementations and calls. How implementations inherit contracts, and whether they may strengthen guarantees, remains to be defined.

**Special function forms (frontend).** Expanding `#[precondition(x < 100)] const fn f(x: u32) -> u32 { x }` calls non-constant helper functions. Extracting `#[postcondition(|r| r > 0)] async fn f() -> u32 { 1 }` encounters unsupported coroutine types. Supporting these forms requires accommodation of constant contexts and of the value obtained when an async function completes, respectively.
