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
