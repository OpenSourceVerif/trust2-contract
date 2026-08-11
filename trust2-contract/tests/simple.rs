use trust2_contract::prelude::*;

#[expect(dead_code)]
#[precondition(x < 16)]
#[postcondition(|x2| x2 >= x)]
fn square(x: u8) -> u8 {
    x * x
}

#[expect(dead_code)]
#[invariant(self.start <= self.end)]
struct RefRange<'a, T: PartialOrd> {
    start: &'a T,
    end: &'a T,
}

#[expect(dead_code)]
#[postcondition(|b| forall!(|i: usize| implies(i + 1 < a.len(), b[i] <= b[i + 1])))]
fn to_sorted<T: Ord + Clone>(a: &[T]) -> Vec<T> {
    if a.len() <= 1 {
        return a.into();
    }

    let (left, right) = a.split_at(a.len() / 2);
    let mut left = to_sorted(left).into_iter().peekable();
    let mut right = to_sorted(right).into_iter().peekable();

    let mut b = Vec::with_capacity(a.len());
    loop {
        match (left.peek(), right.peek()) {
            (Some(x), Some(y)) => {
                b.push(if x <= y {
                    left.next().unwrap()
                } else {
                    right.next().unwrap()
                });
            }
            (Some(_), None) => {
                b.extend(left);
                break;
            }
            (None, Some(_)) => {
                b.extend(right);
                break;
            }
            (None, None) => unreachable!(),
        }
    }

    b
}

#[expect(dead_code)]
fn use_assert() {
    #[precondition(x <= 25)]
    fn decuple(x: u8) -> u8 {
        (x << 3) + (x << 1)
    }

    contract_assert!(decuple(9) == 90);
}
