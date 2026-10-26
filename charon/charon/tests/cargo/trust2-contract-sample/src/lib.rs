use trust2_contract::prelude::*;

#[precondition(true)]
#[postcondition(|c| c >= a)]
#[postcondition(|c| c >= b)]
pub fn max(a: u64, b: u64) -> u64 {
    if a > b { a } else { b }
}

#[postcondition(|c| c <= a && c <= b)]
pub fn min(a: u64, b: u64) -> u64 {
    if a < b { a } else { b }
}

#[invariant(self.start <= self.end)]
pub struct RefRange<'a, T: PartialOrd> {
    start: &'a T,
    end: &'a T,
}

#[postcondition(|b| forall!(|i: usize| implies(i + 1 < a.len(), b[i] <= b[i + 1])))]
pub fn to_sorted(a: &[i32]) -> Vec<i32> {
    vec![]
}

#[expect(dead_code)]
fn use_assert() {
    #[precondition(x <= 25)]
    fn decuple(x: u8) -> u8 {
        (x << 3) + (x << 1)
    }

    contract_assert!(decuple(9) == 90);
}

#[precondition(*p < u64::MAX)]
#[postcondition(|_| *p == *old(p) + 1)]
pub fn increment(p: &mut u64) {
    *p += 1;
}

#[postcondition(|_| forall!(|i: usize| implies(i < v.len(), v[i] == old(v)[i] / 2)))]
pub fn halve_all(v: &mut [u64]) {
    for x in v.iter_mut() {
        *x /= 2;
    }
}
