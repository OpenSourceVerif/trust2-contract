pub fn mutable_borrow() -> i32 {
    let mut x = 0;
    let r = &mut x;
    *r = 1;
    x
}
