//! Whether Enter submits the line or opens a continuation.
//!
//! Uses flyline's `command_acceptance` under the hood.
use reedline::{ValidationResult, Validator};

use crate::grammar::command_acceptance::will_bash_accept_buffer;

#[derive(Default)]
pub struct BashValidator;

impl Validator for BashValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if line.trim().is_empty() || will_bash_accept_buffer(line) {
            ValidationResult::Complete
        } else {
            ValidationResult::Incomplete
        }
    }
}
