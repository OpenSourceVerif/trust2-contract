use prusti_contracts::*;

pub fn branch(flag: bool) {
    let mut a = 1;
    let mut b = 2;
    let r = if flag { &mut a } else { &mut b };
    *r = 9;
    if flag { assert!(a == 9); } else { assert!(b == 9); }
}

