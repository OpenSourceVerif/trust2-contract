//! Uses of `old` that type-check but have no well-defined entry value.
use trust2_contract::prelude::*;

pub struct Pair<'a> {
    pub first: &'a mut u32,
}

// The return value has no entry state.
#[postcondition(|r| *r == *old(r))]
pub fn returned(p: &mut u32) -> &mut u32 {
    p
}

// Neither has a quantified variable.
#[postcondition(|_| forall!(|q: &mut u32| *old(q) == 0))]
pub fn quantified(p: &mut u32) {
    let _ = p;
}

// Fields are not supported yet.
#[postcondition(|_| *pair.first == *old(pair.first))]
pub fn field(pair: &mut Pair) {
    let _ = pair;
}

#[precondition(*trust2_contract::internal::old(&p) == 0)]
pub fn in_precondition(p: &mut u32) {
    let _ = p;
}

pub fn in_assertion(p: &mut u32) {
    contract_assert!(*trust2_contract::internal::old(&p) == 0);
}
