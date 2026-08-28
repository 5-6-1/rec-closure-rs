// async recursive closures are not supported.

use rec_closure::rec_closure;

#[rec_closure]
fn main() {
    let f = async |n: i32| -> i32 {
        if n <= 1 {
            1
        } else {
            n * f(n - 1)
        }
    };
    let _ = f(1);
}
