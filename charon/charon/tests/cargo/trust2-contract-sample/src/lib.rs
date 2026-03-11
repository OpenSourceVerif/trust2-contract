use trust2_contract::{invariant, postcondition, precondition};

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

#[postcondition(|b| forall(|i: usize| implies(i + 1 < a.len(), b[i] <= b[i + 1])))]
pub fn to_sorted(a: &[i32]) -> Vec<i32> {
    vec![]
}
