pub fn entry() {}

pub fn precondition<T: Fn() -> bool>(_: T) {}

pub fn postcondition<T, U: Fn(T) -> bool>(_: U) {}
