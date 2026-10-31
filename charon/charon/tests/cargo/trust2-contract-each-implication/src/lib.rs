//! `==>` and `each!`, extracted with the same flags as `cargo verify`.
use trust2_contract::prelude::*;

#[postcondition(|r| x > 0 ==> r >= x)]
pub fn implication(x: i32) -> i32 {
    x
}

#[postcondition(|r| a < b ==> b < c ==> r == 1)]
pub fn right_associative(a: u8, b: u8, c: u8) -> u8 {
    let _ = (a, b, c);
    1
}

// The consequent is only evaluated when the antecedent holds, so this indexing is in bounds.
#[precondition(forall!(|i: usize| i < v.len() ==> v[i] > 0))]
pub fn guarded_index(v: &[u8]) {
    let _ = v;
}

#[postcondition(|r| each!(i: u8; i < n; i < r))]
pub fn each_guard(n: u8) -> u8 {
    n
}

#[postcondition(|r| each!(i: u8 in 0..=9; i <= r))]
pub fn each_closed_range(x: u8) -> u8 {
    let _ = x;
    9
}

#[precondition(each!(i: usize in 0..v.len(); v[i] > 0))]
pub fn each_half_open_range(v: &[u8]) {
    let _ = v;
}

#[precondition(each!(i: u8 in 0..10; each!(j: u8 in 0..i; j < i)))]
pub fn each_nested() {}

#[postcondition(|r| x > 0 ==> each!(i: u8 in 0..x; i < r))]
pub fn each_under_implication(x: u8) -> u8 {
    x
}

// `old` inside a quantifier: its capture goes through a temporary with `--precise-drops`.
#[postcondition(|_| forall!(|i: usize| i < v.len() ==> v[i] == old(v)[i] / 2))]
pub fn old_in_forall(v: &mut [u64]) {
    let _ = v;
}

#[postcondition(|_| each!(i: usize in 0..v.len(); v[i] == old(v)[i] / 2))]
pub fn old_in_each(v: &mut [u64]) {
    let _ = v;
}

// Two quantifier levels capturing outer variables.
#[postcondition(|r| exists!(|x: u8| x == r && x == n))]
pub fn outer_variables(n: u8) -> u8 {
    n
}

pub fn statements(n: u8) {
    contract_assert!(n > 0 ==> n >= 1);
    contract_assume!(each!(i: u8 in 0..n; i < n));
}

#[invariant(self.low > 0 ==> self.high > 0)]
pub struct Range {
    low: u8,
    high: u8,
}
