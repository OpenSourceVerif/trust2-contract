use trust2_contract::{precondition, postcondition};

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
