pub mod ast;
pub mod lexer;
pub mod parser;
pub mod evaluator;

pub use ast::*;
pub use lexer::{Lexer, Token};
pub use parser::Parser;
pub use evaluator::{Evaluator, OrchestratorHandler};

pub fn run_script<H: OrchestratorHandler>(code: &str, handler: &mut H) -> Result<(), String> {
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer);
    let stmts = parser.parse_program()?;
    let mut evaluator = Evaluator::new();
    evaluator.execute_program(&stmts, handler)
}
