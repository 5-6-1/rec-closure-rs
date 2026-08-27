//! Final lowering template for the recursive-closure macro, single-threaded.
//!
//! User source, no type annotations — like a native closure:
//!
//! ```text
//! let fact = move |n| if n <= 1 { 1 } else { n * fact(n - 1) };
//! ```
//!
//! The macro rewrites it into the inline shape below. Design points:
//!
//! - User code carries no annotations, but the generated slot line is a
//!   required inference anchor: the closure body infers the recursive self
//!   from the slot type, so that type must be spelled out before the closure
//!   exists (a fully inferred slot fails with E0282). The `Slot` aliases only
//!   keep clippy's `type_complexity` quiet; the explicit `'a` keeps the
//!   trait-object lifetime inferable instead of `'static`.
//! - The recursive self is bound as a `&dyn Fn` reference (`Copy`), so a
//!   nested closure captures only a copy; see `nested.rs` for nesting.
//! - Every generated identifier carries a per-fn sequence number
//!   (`__rec_{N}_{role}`), so several closures in one fn never collide.
//! - `move` is optional. A non-`move` closure borrows the internal slot, so
//!   it must be expanded inline (same scope) and cannot escape that scope;
//!   `move` closures use the same inline shape, so one code path serves both.
//! - `OnceCell` + `Weak<rec>`: no runtime borrow checks, no reference cycle.
//!
//! Multithreaded variant: `sync.rs`. Zero-allocation typed variant: `typed.rs`.

use std::cell::OnceCell;
use std::rc::{Rc, Weak};

type Slot<'a, A, R> = Rc<OnceCell<Weak<dyn Fn(A) -> R + 'a>>>;
type Slot2<'a, A, B, R> = Rc<OnceCell<Weak<dyn Fn(A, B) -> R + 'a>>>;

fn main() {
    // 1. move closure capturing `x`
    let x = 1;
    let __rec_0_slot: Slot<'_, _, _> = Rc::new(OnceCell::new());
    let __rec_0_slot_ref = Rc::clone(&__rec_0_slot);
    let __rec_0_rec = Rc::new(move |n| {
        let __rec_0_strong = __rec_0_slot_ref.get().unwrap().upgrade().unwrap();
        let __rec_0_self: &dyn Fn(_) -> _ = &*__rec_0_strong;
        if n <= 1 { x } else { n * __rec_0_self(n - 1) }
    });
    let __rec_0_weak: Weak<dyn Fn(_) -> _> =
        Rc::downgrade(&(__rec_0_rec.clone() as Rc<dyn Fn(_) -> _>));
    __rec_0_slot.set(__rec_0_weak).ok().unwrap();
    let fact = __rec_0_rec;
    assert_eq!(fact(5), 120);
    println!("{}", fact(5));

    // 2. non-move closure: borrows `y` and the slot; `y` stays usable after
    let y = 10;
    let __rec_1_slot: Slot<'_, _, _> = Rc::new(OnceCell::new());
    let __rec_1_slot_ref = Rc::clone(&__rec_1_slot);
    let __rec_1_rec = Rc::new(|n| {
        let __rec_1_strong = __rec_1_slot_ref.get().unwrap().upgrade().unwrap();
        let __rec_1_self: &dyn Fn(_) -> _ = &*__rec_1_strong;
        if n <= 0 { y } else { __rec_1_self(n - 1) + 1 }
    });
    let __rec_1_weak: Weak<dyn Fn(_) -> _> =
        Rc::downgrade(&(__rec_1_rec.clone() as Rc<dyn Fn(_) -> _>));
    __rec_1_slot.set(__rec_1_weak).ok().unwrap();
    let countdown = __rec_1_rec;
    assert_eq!(countdown(3), 13);
    assert_eq!(y, 10);
    println!("{}", countdown(3));

    // 3. multi-arg with `mut` patterns
    let __rec_2_slot: Slot2<'_, _, _, _> = Rc::new(OnceCell::new());
    let __rec_2_slot_ref = Rc::clone(&__rec_2_slot);
    let __rec_2_rec = Rc::new(move |mut a, mut b| {
        let __rec_2_strong = __rec_2_slot_ref.get().unwrap().upgrade().unwrap();
        let __rec_2_self: &dyn Fn(_, _) -> _ = &*__rec_2_strong;
        if a < b {
            std::mem::swap(&mut a, &mut b);
        }
        if b != 0 { __rec_2_self(b, a % b) } else { a }
    });
    let __rec_2_weak: Weak<dyn Fn(_, _) -> _> =
        Rc::downgrade(&(__rec_2_rec.clone() as Rc<dyn Fn(_, _) -> _>));
    __rec_2_slot.set(__rec_2_weak).ok().unwrap();
    let gcd = __rec_2_rec;
    assert_eq!(gcd(12, 32), 4);
    println!("{}", gcd(12, 32));
}
