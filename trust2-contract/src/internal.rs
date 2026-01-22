pub fn entry() {}

pub fn precondition<T: Fn() -> bool + Copy>(_: T) {}

pub fn postcondition<T, U: Fn(T) -> bool + Copy>(_: U) {}
