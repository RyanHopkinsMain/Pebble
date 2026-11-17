//! Pebble: a small, dynamically typed scripting language with first-class
//! functions and closures.
//!
//! ```text
//! source --lexer--> tokens --parser--> syntax tree --interpreter--> output
//! ```

pub mod ast;
pub mod builtins;
pub mod error;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod value;

use std::io::Write;

pub use error::Error;
pub use interpreter::Interpreter;
pub use value::Value;

/// Parse source text into a program.
pub fn parse(source: &str) -> Result<Vec<ast::Stmt>, Error> {
    parser::parse(lexer::tokenize(source)?)
}

/// A fresh interpreter with the builtins and the Pebble-written prelude loaded.
pub fn interpreter(out: Box<dyn Write>) -> Interpreter {
    let mut interp = Interpreter::new(out);
    let prelude = parse(builtins::PRELUDE).expect("prelude parses");
    interp.run(&prelude).expect("prelude runs");
    interp
}

/// Run a whole program, writing `print` output to `out`.
pub fn run(source: &str, out: Box<dyn Write>) -> Result<(), Error> {
    let program = parse(source)?;
    interpreter(out).run(&program).map(|_| ())
}
