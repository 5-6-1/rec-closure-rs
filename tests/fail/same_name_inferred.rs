use rec_closure::rec_closure;

#[rec_closure]
fn main() {
    let f = |n| {
        let f = |m| if m == 0 { 1 } else { f(m - 1) };
        f(n)
    };
    let _ = f(1);
}
