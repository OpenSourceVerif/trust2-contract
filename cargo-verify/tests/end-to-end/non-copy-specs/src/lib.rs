use trust2_contract::prelude::*;

pub struct BoxedInt {
    pub val: i32,
}

pub struct NonCopyPair {
    pub a: i32,
    pub b: i32,
}

#[precondition({
    let first = x;
    let second = x;
    first.val < 100 && second.val >= 0
})]
#[postcondition(|res| {
    let orig = x;
    let out1 = res;
    let out2 = res;
    out1.val == orig.val + 1
        && out2.val > orig.val
        && forall!(|q: BoxedInt| {
            let q1 = q;
            let q2 = q;
            q1.val == q2.val
        })
})]
pub fn inc_non_copy(x: BoxedInt) -> BoxedInt {
    BoxedInt { val: x.val + 1 }
}

#[precondition(p.a >= 0 && p.b >= 0 && p.a < 1000)]
#[postcondition(|res| {
    let before = p;
    let after1 = res;
    let after2 = res;
    after1.a == before.a + 1 && after2.b == before.b
})]
pub fn bump_first_reusing_non_copy(p: NonCopyPair) -> NonCopyPair {
    let next = NonCopyPair { a: p.a + 1, b: p.b };
    contract_assert!({
        let copy1 = next;
        let copy2 = next;
        let mut clamp = copy1.b;
        if clamp < 0 {
            clamp = 0;
        }
        copy1.a >= 1 && clamp == copy2.b
    });
    next
}
