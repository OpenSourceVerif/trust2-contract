use trust2_contract::{postcondition, precondition};

#[expect(dead_code)]
#[precondition(x < 16)]
#[postcondition(|x2| x2 >= x)]
fn square(x: u8) -> u8 {
    x * x
}
