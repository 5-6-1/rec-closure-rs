//! Shared test fixtures.
//!
//! `dead_code`: individual test crates exercise only some fields.

#[allow(dead_code)]
pub struct Tree {
    pub value: i32,
    pub children: Vec<Tree>,
}
