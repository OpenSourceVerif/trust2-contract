{{#rule contract.old}}

# Old-state Expressions

{{#syntax OldExpression}}

{{#rule contract.old.intro}}

## Semantics

An old-state expression in a postcondition refers to the function's pre-state: the state on entry to the function. Its purpose is to relate a returned state to an earlier one.

`old(p)` takes a mutable reference parameter `p` of the function and denotes that reference as it was on entry, so `*old(p)` is the value `p` pointed to on entry, while `*p` is the value on return. `old` is a marker on a parameter, not a snapshot operation on arbitrary Rust expressions: `old(*p)`, `old(x + 1)` or `old(v.len())` are not accepted. Only `&mut` parameters have entry values that differ from their return values; other parameters already denote their entry values.

{{#rule contract.old.example}}

## Example

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(*p < u32::MAX)]
#[postcondition(|_| *p == *old(p) + 1)]
pub fn increment(p: &mut u32) {
    *p += 1;
}

#[postcondition(|_| forall!(|i: usize| implies(i < v.len(), v[i] == old(v)[i] / 2)))]
pub fn halve_all(v: &mut [u32]) {
    for x in v.iter_mut() {
        *x /= 2;
    }
}
```

`old` may appear inside quantifiers nested in a postcondition, and applies to unsized pointees such as slices. The postcondition macro rewrites `old(p)` to `old(&p)`, so the closure captures `p` by shared reference and stays `Fn + Copy`; a non-`&mut` argument is a type error.

{{#rule contract.old.boundary}}

## Implementation boundaries <a id="contract.old.boundary-title"></a>

- **Frontend `rust-llbc`**: extracts `old(p)` as a `SpecOld` call whose argument is the postcondition's capture of `p`. It rejects `old` outside postconditions, and arguments other than a parameter of the function, such as the returned reference, a quantified variable, or a field `old(s.f)`.
- **Backend `llbc-why3`**: old-state translation is not implemented; `SpecOld` and mutable borrows are not yet translated.

### Remaining work

**Entry values of mutable parameters (backend).** For `increment` above, the backend needs to translate `*old(p)` to the entry value of `*p` and `*p` to its return value, using its pair representation of mutable references.

**Fields and compound values (frontend and backend).** The entry value of `*s.f` for `s: &mut S` with a field `f: &mut T`, and old state for data reachable through several references, remain to be defined.
