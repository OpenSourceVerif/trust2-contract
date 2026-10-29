// Self-contained regression cases for the supported CFG/borrowck fragment.
pub fn repeated_write() -> i32 {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    *r = 2;
    x
}
pub fn sequential_borrows() -> i32 {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    let s = &mut x;
    *s = 2;
    x
}
pub fn goto_borrow() -> i32 {
    let mut x = loop { break 0; };
    let r = &mut x;
    loop { *r = 1; break; }
    x
}
pub fn two_shared() -> i32 {
    let x = 7;
    let r = &x;
    let s = &x;
    let _a = *r;
    *s
}
pub fn unit_result() {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
}
pub fn branch(c: bool) -> i32 {
    let mut x = 0;
    if c { x = 1; }
    x
}
pub fn cycle(c: bool) -> i32 {
    let mut x = 0;
    while c { x = 1; }
    x
}
pub fn reborrow() -> i32 {
    let mut x = 0;
    let r = &mut x;
    let s = &mut *r;
    *s = 1;
    x
}

// Normal cleanup removes this unit local and renumbers the borrow's locals.
pub fn removed_unit_before_borrow() -> i32 {
    let _unused = ();
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    x
}
