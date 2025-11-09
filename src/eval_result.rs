//! Result an error types used for evaluation functions.

use std::{error::Error, fmt};

/// A Result type for the shell's eval functions.
pub type EvalResult = Result<(), Box<dyn Error>>;

/// An error type for any custom error during evaluation.
#[derive(Debug)]
pub struct EvalError {
    message: String,
}

impl EvalError {
    pub fn new(message: String) -> EvalError {
        EvalError { message }
    }

    pub fn from_str(message: &str) -> EvalError {
        let message = String::from(message);
        EvalError { message }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for EvalError {}

/// Creates an `EvalResult` with a given `EvalError`.
pub fn eval_result(eval_error: EvalError) -> EvalResult {
    let boxed_self: Box<dyn Error> = Box::new(eval_error);
    Err(boxed_self)
}
