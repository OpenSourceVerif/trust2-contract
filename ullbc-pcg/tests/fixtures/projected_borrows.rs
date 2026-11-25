pub struct Pair {
    pub x: i32,
    pub y: i32,
}

pub fn change(mut p: Pair) -> Pair {
    let r = &mut p.x;
    *r = 7;
    p
}

pub fn disjoint(mut p: Pair) -> Pair {
    let x = &mut p.x;
    let y = &mut p.y;
    *x = 3;
    *y = 4;
    p
}

pub fn nested(mut p: (Pair, i32)) -> (Pair, i32) {
    let r = &mut p.0.x;
    let s = &mut *r;
    *s = 7;
    *r = p.1;
    p
}

pub fn choose(mut p: Pair, left: bool) -> Pair {
    if left {
        let r = &mut p.x;
        *r = 7;
    } else {
        let r = &mut p.y;
        *r = 8;
    }
    p
}

pub fn borrow_choice(mut p: Pair, left: bool) -> Pair {
    let r = if left { &mut p.x } else { &mut p.y };
    *r = 7;
    p
}

pub fn loop_fields(mut p: Pair, mut n: u32) -> Pair {
    while n != 0 {
        let r = &mut p.x;
        *r = p.y;
        n -= 1;
    }
    p
}

pub fn read_arg(r: &i32) -> i32 {
    *r
}

pub fn return_arg(r: &mut i32) -> &mut i32 {
    *r = 7;
    r
}

pub fn shared_field(p: Pair) -> i32 {
    let r = &p.x;
    let s = &p.y;
    *r + *s
}

pub fn loop_borrow(mut p: Pair, mut n: u32) -> Pair {
    let mut r = &mut p.x;
    while n > 0 {
        *r = 7;
        r = &mut p.y;
        n -= 1;
    }
    *r = 9;
    p
}

pub fn call_two_phase(mut p: Pair) -> Pair {
    let r = return_arg(&mut p.x);
    *r = 8;
    p
}

pub fn call_value(p: Pair) -> i32 {
    read_arg(&p.y)
}


pub fn identity_shared(x: &i32) -> &i32 {
    x
}

pub fn call_ref(p: Pair) -> i32 {
    let r = identity_shared(&p.x);
    *r
}
