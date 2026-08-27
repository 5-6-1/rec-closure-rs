//! Nested recursive closures: a recursive closure whose body defines another
//! recursive closure.
//!
//! Every generated identifier carries a per-fn sequence number
//! (`__rec_{N}_{role}`), so nested expansions never shadow each other:
//! `__rec_0_*` belongs to the outer closure, `__rec_1_*` to the inner one.
//! The recursive self is a `&dyn Fn` reference (`Copy`), so the inner
//! `move` closure captures only a copy and the outer body keeps using its
//! own self afterwards.
//!
//! References to the outer closure from inside the inner body are rewritten
//! to the outer self (`__rec_0_self`), which is equivalent to calling it by
//! name.
//!
//! Expected values: inner(1) = outer(0) = 1, inner(2) = 2·1+1 = 3,
//! inner(3) = 3·3+1 = 10; outer(1) = 1, outer(2) = 2·1+3 = 5,
//! outer(3) = 3·5+10 = 25.

use std::cell::OnceCell;
use std::rc::{Rc, Weak};

type Slot<'a, A, R> = Rc<OnceCell<Weak<dyn Fn(A) -> R + 'a>>>;

fn main() {
    let __rec_0_slot: Slot<'_, _, _> = Rc::new(OnceCell::new());
    let __rec_0_slot_ref = Rc::clone(&__rec_0_slot);
    let __rec_0_rec = Rc::new(move |n: i32| -> i32 {
        let __rec_0_strong = __rec_0_slot_ref.get().unwrap().upgrade().unwrap();
        let __rec_0_self: &dyn Fn(_) -> _ = &*__rec_0_strong;
        // ---- inner expansion (inline, __rec_1_*) ----
        let __rec_1_slot: Slot<'_, _, _> = Rc::new(OnceCell::new());
        let __rec_1_slot_ref = Rc::clone(&__rec_1_slot);
        let __rec_1_rec = Rc::new(move |m: i32| -> i32 {
            let __rec_1_strong = __rec_1_slot_ref.get().unwrap().upgrade().unwrap();
            let __rec_1_self: &dyn Fn(_) -> _ = &*__rec_1_strong;
            // inner body: itself -> __rec_1_self, outer -> __rec_0_self
            if m <= 1 { __rec_0_self(m) } else { m * __rec_1_self(m - 1) + __rec_0_self(0) }
        });
        let __rec_1_weak: Weak<dyn Fn(_) -> _> =
            Rc::downgrade(&(__rec_1_rec.clone() as Rc<dyn Fn(_) -> _>));
        __rec_1_slot.set(__rec_1_weak).ok().unwrap();
        let inner = __rec_1_rec;
        // ---- outer body ----
        if n <= 1 { 1 } else { n * __rec_0_self(n - 1) + inner(n) }
    });
    let __rec_0_weak: Weak<dyn Fn(_) -> _> =
        Rc::downgrade(&(__rec_0_rec.clone() as Rc<dyn Fn(_) -> _>));
    __rec_0_slot.set(__rec_0_weak).ok().unwrap();
    let outer = __rec_0_rec;

    assert_eq!(outer(3), 25);
    println!("{}", outer(3));
}
