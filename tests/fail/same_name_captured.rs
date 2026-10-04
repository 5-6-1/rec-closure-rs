use rec_closure::rec_closure;

#[rec_closure]
fn main() {
    let base = 1;
    let f = |n: i32| -> i32 {
        let f = |m: i32| -> i32 { if m == 0 { base } else { f(m - 1) } };
        f(n)
    };
    let _ = f(1);
}
