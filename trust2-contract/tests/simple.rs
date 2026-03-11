use trust2_contract::{invariant, postcondition, precondition};

#[expect(dead_code)]
#[precondition(x < 16)]
#[postcondition(|x2| x2 >= x)]
fn square(x: u8) -> u8 {
    x * x
}

#[expect(dead_code)]
#[invariant(self.start <= self.end)]
struct RefRange<'a, T: PartialOrd> {
    start: &'a T,
    end: &'a T,
}

#[expect(dead_code)]
#[postcondition(|b| forall(|i: usize| implies(i + 1 < a.len(), b[i] <= b[i + 1])))]
fn to_sorted(a: &[i32]) -> Vec<i32> {
    vec![]
}
