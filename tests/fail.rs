//! Compile-fail matrix: the macro's explicit rejection contracts.

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/fail/*.rs");
}
