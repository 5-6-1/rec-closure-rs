//! Multithreaded variant of the recursive-closure lowering.
//!
//! The only differences from the single-threaded template (`optimized.rs`):
//! `Arc` instead of `Rc`, `OnceLock` instead of `OnceCell`, the trait object
//! carries `+ Send + Sync`, and the self reference is `&(dyn Fn + Send + Sync)`
//! (the closure captures it across the thread boundary, so it must be
//! `Send`). The closure and everything it captures must be `Send + Sync`;
//! `thread::spawn` demands that anyway.

use std::sync::{Arc, OnceLock, Weak};

type Slot<'a, A, R> = Arc<OnceLock<Weak<dyn Fn(A) -> R + Send + Sync + 'a>>>;

fn main() {
    let base = 1;
    let __rec_0_slot: Slot<'_, _, _> = Arc::new(OnceLock::new());
    let __rec_0_slot_ref = Arc::clone(&__rec_0_slot);
    let __rec_0_rec = Arc::new(move |n| {
        let __rec_0_strong = __rec_0_slot_ref.get().unwrap().upgrade().unwrap();
        let __rec_0_self: &(dyn Fn(_) -> _ + Send + Sync) = &*__rec_0_strong;
        if n <= 1 { base } else { n * __rec_0_self(n - 1) }
    });
    let __rec_0_weak: Weak<dyn Fn(_) -> _ + Send + Sync> =
        Arc::downgrade(&(__rec_0_rec.clone() as Arc<dyn Fn(_) -> _ + Send + Sync>));
    __rec_0_slot.set(__rec_0_weak).ok().unwrap();
    let fact = __rec_0_rec;

    assert_eq!(fact(5), 120);
    println!("main: {}", fact(5));

    let handle = std::thread::spawn(move || fact(6));
    let result = handle.join().unwrap();
    assert_eq!(result, 720);
    println!("thread: {}", result);
}
