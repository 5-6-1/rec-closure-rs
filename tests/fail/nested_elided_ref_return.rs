// Container-nested elided reference returns are rejected with a hint to use
// explicit lifetimes.

use rec_closure::rec_closure;

#[rec_closure]
fn main() {
    let marker = "!";
    let f = |s: &str| -> Option<&str> {
        if s.is_empty() {
            None
        } else {
            f(&s[1..])
        }
    };
    let _ = f(marker);
}
