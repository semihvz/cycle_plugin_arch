pub mod ast;
pub mod evaluator;
pub mod lexer;
pub mod parser;

pub use ast::*;
pub use evaluator::{Evaluator, OrchestratorHandler};
pub use lexer::Lexer;
pub use parser::Parser;

pub fn run_script<H: OrchestratorHandler>(code: &str, handler: &mut H) -> Result<(), String> {
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer);
    let stmts = parser.parse_program()?;
    let mut evaluator = Evaluator::new();
    evaluator.eval_program(&stmts, handler)
}
