//! Typed (zero-allocation) path of the recursive-closure lowering.
//!
//! When the user closure carries complete type annotations, the macro can
//! expand to the `fix_fn` scheme: the recursive self is passed as a
//! `&dyn HideFn` parameter (Y-combinator style), so there is no slot, no
//! `OnceCell`, and no `Weak` — zero heap allocation per call, only one
//! dynamic dispatch per recursion level.
//!
//! Contrast with `optimized.rs`: that is the inferred path (user code has no
//! annotations), which needs the `Rc + OnceCell` machinery because a trait
//! definition requires concrete signatures and cannot use `_` placeholders.
//!
//! Both paths keep the native-closure look: the user only writes
//! `move |n: i32| -> i32 { ... fact(n - 1) ... }` and the macro substitutes
//! `fact` with the wrapped self.

use std::cell::Cell;
use std::thread;

fn main() {
    // 1. single-threaded, move
    let fact = {
        trait HideFn {
            fn call(&self, n: i32) -> i32;
        }
        struct HideFnImpl<F: Fn(&dyn HideFn, i32) -> i32>(F);
        impl<F: Fn(&dyn HideFn, i32) -> i32> HideFn for HideFnImpl<F> {
            #[inline]
            fn call(&self, n: i32) -> i32 {
                self.0(self, n)
            }
        }
        let inner = HideFnImpl(move |self_fn, n: i32| -> i32 {
            let self_fn = |n: i32| self_fn.call(n);
            if n <= 1 { 1 } else { n * self_fn(n - 1) }
        });
        move |n: i32| -> i32 { inner.call(n) }
    };
    assert_eq!(fact(5), 120);
    println!("{}", fact(5));

    // 2. multi-arg with `mut` patterns
    let gcd = {
        trait HideFn {
            fn call(&self, a: i32, b: i32) -> i32;
        }
        struct HideFnImpl<F: Fn(&dyn HideFn, i32, i32) -> i32>(F);
        impl<F: Fn(&dyn HideFn, i32, i32) -> i32> HideFn for HideFnImpl<F> {
            #[inline]
            fn call(&self, a: i32, b: i32) -> i32 {
                self.0(self, a, b)
            }
        }
        let inner = HideFnImpl(move |self_fn, mut a: i32, mut b: i32| -> i32 {
            let self_fn = |a: i32, b: i32| self_fn.call(a, b);
            if a < b {
                std::mem::swap(&mut a, &mut b);
            }
            if b != 0 { self_fn(b, a % b) } else { a }
        });
        move |a: i32, b: i32| -> i32 { inner.call(a, b) }
    };
    assert_eq!(gcd(12, 32), 4);
    println!("{}", gcd(12, 32));

    // 3. non-move borrowing an outer variable (`y` lives in fn scope, so the
    //    block-typed inner may borrow it; `y` stays usable afterwards)
    let y = 2;
    let count = {
        trait HideFn {
            fn call(&self, n: i32) -> i32;
        }
        struct HideFnImpl<F: Fn(&dyn HideFn, i32) -> i32>(F);
        impl<F: Fn(&dyn HideFn, i32) -> i32> HideFn for HideFnImpl<F> {
            #[inline]
            fn call(&self, n: i32) -> i32 {
                self.0(self, n)
            }
        }
        let inner = HideFnImpl(|self_fn, n: i32| -> i32 {
            let self_fn = |n: i32| self_fn.call(n);
            if n <= 0 { y } else { self_fn(n - 1) + 1 }
        });
        move |n: i32| -> i32 { inner.call(n) }
    };
    assert_eq!(count(3), 5);
    assert_eq!(y, 2);
    println!("{}", count(3));

    // 4. zero parameters, `Cell` interior mutability for state
    let countdown = {
        trait HideFn {
            fn call(&self) -> i32;
        }
        struct HideFnImpl<F: Fn(&dyn HideFn) -> i32>(F);
        impl<F: Fn(&dyn HideFn) -> i32> HideFn for HideFnImpl<F> {
            #[inline]
            fn call(&self) -> i32 {
                self.0(self)
            }
        }
        let cell = Cell::new(3);
        let inner = HideFnImpl(move |self_fn| -> i32 {
            let self_fn = || self_fn.call();
            let n = cell.get();
            if n <= 0 {
                0
            } else {
                cell.set(n - 1);
                self_fn()
            }
        });
        move || -> i32 { inner.call() }
    };
    assert_eq!(countdown(), 0);
    println!("{}", countdown());

    // 5. multithreaded: `HideFn: Send + Sync`, callable from another thread
    let base = 1;
    let fact_sync = {
        trait HideFn: Send + Sync {
            fn call(&self, n: i32) -> i32;
        }
        struct HideFnImpl<F: Fn(&dyn HideFn, i32) -> i32 + Send + Sync>(F);
        impl<F: Fn(&dyn HideFn, i32) -> i32 + Send + Sync> HideFn for HideFnImpl<F> {
            #[inline]
            fn call(&self, n: i32) -> i32 {
                self.0(self, n)
            }
        }
        let inner = HideFnImpl(move |self_fn, n: i32| -> i32 {
            let self_fn = |n: i32| self_fn.call(n);
            if n <= 1 { base } else { n * self_fn(n - 1) }
        });
        move |n: i32| -> i32 { inner.call(n) }
    };
    let handle = thread::spawn(move || fact_sync(6));
    let result = handle.join().unwrap();
    assert_eq!(result, 720);
    println!("{}", result);
}
