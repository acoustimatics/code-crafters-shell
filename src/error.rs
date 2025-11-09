//! Result an error types used for evaluation functions.

use std::{error::Error, fmt};

/// An error type for any custom error during evaluation.
#[derive(Debug)]
pub struct EvalError {
    message: String,
}

impl EvalError {
    pub fn new(message: String) -> EvalError {
        EvalError { message }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for EvalError {}
