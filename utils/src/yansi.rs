use yansi::Condition;

use std::sync::atomic::AtomicU8;

pub const STDOUT_IS_TTY_AND_COLOR: Condition = Condition(stdout_is_tty_and_color);

pub const STDOUT_IS_TTY_AND_COLOR_LIVE: Condition = Condition(stdout_is_tty_and_color_live);

pub fn stdout_is_tty_and_color() -> bool {
    static IS_TTY: CachedBool = CachedBool::new();
    IS_TTY.get_or_init(stdout_is_tty_and_color_live)
}

pub fn stdout_is_tty_and_color_live() -> bool {
    Condition::stdout_is_tty() && Condition::clicolor() && Condition::no_color()
}

pub const STDERR_IS_TTY_AND_COLOR: Condition = Condition(stderr_is_tty_and_color);

pub const STDERR_IS_TTY_AND_COLOR_LIVE: Condition = Condition(stderr_is_tty_and_color_live);

pub fn stderr_is_tty_and_color() -> bool {
    static IS_TTY: CachedBool = CachedBool::new();
    IS_TTY.get_or_init(stderr_is_tty_and_color_live)
}

pub fn stderr_is_tty_and_color_live() -> bool {
    Condition::stderr_is_tty() && Condition::clicolor() && Condition::no_color()
}

#[allow(unused)]
#[repr(transparent)]
pub struct CachedBool(AtomicU8);

#[allow(unused)]
impl CachedBool {
    const TRUE: u8 = 1;
    const UNINIT: u8 = 2;
    const INITING: u8 = 3;

    pub const fn new() -> Self {
        CachedBool(AtomicU8::new(Self::UNINIT))
    }

    pub fn get_or_init(&self, f: impl FnOnce() -> bool) -> bool {
        use core::sync::atomic::Ordering::*;

        match self
            .0
            .compare_exchange(Self::UNINIT, Self::INITING, AcqRel, Relaxed)
        {
            Ok(_) => {
                let new_value = f();
                self.0
                    .store(new_value as u8 /* false = 0, true = 1 */, Release);
                new_value
            }
            Err(Self::INITING) => {
                let mut value;
                while {
                    value = self.0.load(Acquire);
                    value
                } == Self::INITING
                {
                    std::thread::yield_now();
                }

                value == Self::TRUE
            }
            Err(value) => value == Self::TRUE,
        }
    }
}

impl Default for CachedBool {
    fn default() -> Self {
        CachedBool::new()
    }
}
