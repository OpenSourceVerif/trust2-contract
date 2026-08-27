use trust2_contract::prelude::*;

#[postcondition(|c| c >= a)]
#[postcondition(|c| c >= b)]
fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

#[precondition(a >= b)]
#[postcondition(|c| c + b == a)]
fn strict_sub(a: u32, b: u32) -> u32 {
    a - b
}

#[precondition(x < 16)]
#[postcondition(|x2| x2 >= x)]
fn square(x: u8) -> u8 {
    x * x
}

fn main() {}
