#![allow(unused_assignments)]

pub fn mutable_borrow() -> i32 {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    x
}

pub fn shared_borrow() -> i32 {
    let mut x = 1;
    let r = &x;
    let value = *r;
    x = 2;
    value
}

pub fn moved_reference() -> i32 {
    let mut x = 0;
    let r = &mut x;
    let s = r;
    *s = 3;
    x
}

pub fn scalar_argument(x: i32) -> i32 {
    let y = x;
    y
}

pub fn shared_aliases() -> i32 {
    let x = 7;
    let r = &x;
    let s = r;
    let _first = *r;
    *s
}

pub fn overwrite_reference() -> i32 {
    let mut x = 0;
    let mut y = 1;
    let mut r = &mut x;
    *r = 2;
    r = &mut y;
    *r = 3;
    x
}
