pub mod ast;
pub mod evaluator;

pub use ast::*;
pub use evaluator::{JsonStrategyEvaluator, OrchestratorHandler};

pub fn run_script<H: OrchestratorHandler>(json_str: &str, handler: &mut H) -> Result<(), String> {
    let spec: JsonStrategySpec = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON Strateji Ayrıştırma Hatası: {}", e))?;
    JsonStrategyEvaluator::execute(&spec, handler)
}
