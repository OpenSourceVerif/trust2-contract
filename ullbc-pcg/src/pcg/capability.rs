// Adapted from prusti/pcg at 6fdb05e6377da5316e5388320d2ddf3d13082f6e.
// Source: src/pcg/capabilities.rs. Enum, Debug and partial order retained.
use serde::Serialize;
use std::{
    cmp::Ordering,
    fmt::{Debug, Formatter, Result},
};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Capability {
    /// For borrowed places only: permits reads from the location, but not writes or
    /// drops.
    Read,

    /// For owned places, this capability is used when the place is moved out
    /// of. This capability is used for both owned and borrowed places just before
    /// they are overwritten.
    Write,

    /// Writes and reads are permitted to this place, and the place is not
    /// borrowed.
    Exclusive,

    /// [`Capability::Exclusive`] for everything not through a dereference,
    /// [`Capability::Write`] for everything through a dereference.
    ShallowExclusive,
}
impl Debug for Capability {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Capability::Read => write!(f, "R"),
            Capability::Write => write!(f, "W"),
            Capability::Exclusive => write!(f, "E"),
            Capability::ShallowExclusive => write!(f, "e"),
        }
    }
}

impl PartialOrd for Capability {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if *self == *other {
            return Some(Ordering::Equal);
        }
        match (*self, *other) {
            // Greater relationships
            (
                Capability::Exclusive,
                Capability::ShallowExclusive | Capability::Write | Capability::Read,
            )
            | (Capability::ShallowExclusive, Capability::Write) => Some(Ordering::Greater),

            // Less relationships (inverses of the above)
            (
                Capability::ShallowExclusive | Capability::Write | Capability::Read,
                Capability::Exclusive,
            )
            | (Capability::Write, Capability::ShallowExclusive) => Some(Ordering::Less),

            // All other pairs are incomparable
            _ => None,
        }
    }
}

impl Capability {
    pub fn permits(self, required: Self) -> bool {
        matches!(
            self.partial_cmp(&required),
            Some(Ordering::Equal | Ordering::Greater)
        )
    }
}
