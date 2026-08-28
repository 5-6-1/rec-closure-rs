// A nested recursive closure cannot reuse an enclosing closure's name.

use rec_closure::rec_closure;

#[rec_closure]
fn main() {
    let f = |n: i32| -> i32 {
        let f = |m: i32| -> i32 {
            if m <= 1 {
                1
            } else {
                m * f(m - 1)
            }
        };
        f(n)
    };
    let _ = f(1);
}
