//! Everything that directly interacts with bash
//!
//! `enable -f libreedline_bash.so reedline` loads `reedline-bash` into the bash process.
//!
//! These modules bind bash's internal C symbols and structs.
//! This is currently pinned to bash 5.3

pub mod builtin;
pub mod complete;
pub mod input;
pub mod symbols;
