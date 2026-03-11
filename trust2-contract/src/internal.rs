pub fn entry() {}

pub fn precondition<T: Fn() -> bool + Copy>(_: T) {}

pub fn postcondition<T, U: Fn(T) -> bool + Copy>(_: U) {}

pub trait TypeInvariant {
    fn invariant(&self) -> bool;
}

pub fn forall<T, U: Fn(T) -> bool + Copy>(_: U) -> bool {
    true
}

pub fn exists<T, U: Fn(T) -> bool + Copy>(_: U) -> bool {
    true
}

pub fn implies(_: bool, _: bool) -> bool {
    true
}

pub fn old<T>(x: &mut T) -> &mut T {
    x
}
