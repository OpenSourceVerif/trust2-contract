pub fn entry() {}

pub fn precondition<T: Fn() -> bool + Copy>(_: T) {}

pub fn postcondition<T, U: Fn(T) -> bool + Copy>(_: U) {}

pub trait TypeInvariant {
    fn invariant(&self) -> bool;
}

pub fn contract_assert<T: Fn() -> bool + Copy>(_: T) {}

pub fn contract_assume<T: Fn() -> bool + Copy>(_: T) {}

pub fn forall<T, U: Fn(T) -> bool + Copy>(_: U) -> bool {
    true
}

pub fn exists<T, U: Fn(T) -> bool + Copy>(_: U) -> bool {
    true
}

pub fn implies(_: bool, _: bool) -> bool {
    true
}

/// `old(p)` in a postcondition is rewritten to `old(&p)` by the macro, so the closure
/// only captures `p` by shared reference and remains `Fn + Copy`.
pub fn old<'a, T: ?Sized>(x: &'a &mut T) -> &'a T {
    x
}
