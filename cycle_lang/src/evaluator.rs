use crate::ast::*;
use serde_json::Value as JsonValue;

pub trait OrchestratorHandler {
    fn load_plugin(&mut self, var_name: &str, path: &str) -> Result<(), String>;
    fn start_plugin(&mut self, var_name: &str) -> Result<(), String>;
    fn stop_plugin(&mut self, var_name: &str) -> Result<(), String>;
    fn pin_core(&mut self, var_name: &str, core: usize) -> Result<(), String>;
    fn pipe_stream(&mut self, from_p: &str, from_s: &str, to_p: &str, to_i: &str) -> Result<(), String>;
    fn buy_order(&mut self, symbol: &str, qty: f64, price: f64, leverage: f64) -> Result<(), String>;
    fn sell_order(&mut self, symbol: &str, qty: f64, price: f64, leverage: f64) -> Result<(), String>;
    fn close_position(&mut self, symbol: &str) -> Result<(), String>;
    fn run_sql(&mut self, query: &str) -> Result<String, String>;
    fn call_plugin(&mut self, plugin: &str, method: &str, args: &[Value]) -> Result<Value, String>;
}

pub struct JsonStrategyEvaluator;

impl JsonStrategyEvaluator {
    pub fn execute<H: OrchestratorHandler>(spec: &JsonStrategySpec, handler: &mut H) -> Result<(), String> {
        let name = if !spec.strategy_name.is_empty() {
            &spec.strategy_name
        } else if !spec.name.is_empty() {
            &spec.name
        } else {
            "Unnamed_JSON_Strategy"
        };

        println!("\x1b[96m\x1b[1m[JSON Engine]\x1b[0m STRATEJİ YÜRÜTÜLÜYOR: {}", name);

        // 1. Eklentileri yükle, pin'le ve başlat
        for p in &spec.plugins {
            let plugin_id = p.get_name();
            handler.load_plugin(plugin_id, &p.path)?;
            if let Some(core_id) = p.core {
                handler.pin_core(plugin_id, core_id)?;
            }
            handler.start_plugin(plugin_id)?;
        }

        // 2. Veri akış boru hatlarını bağla (pipe)
        for pipe in &spec.pipes {
            let from_parts: Vec<&str> = pipe.from.split('.').collect();
            let to_parts: Vec<&str> = pipe.to.split('.').collect();

            let from_p = from_parts.get(0).copied().unwrap_or(&pipe.from);
            let from_s = from_parts.get(1).copied().unwrap_or("out");
            let to_p = to_parts.get(0).copied().unwrap_or(&pipe.to);
            let to_i = to_parts.get(1).copied().unwrap_or("in");

            handler.pipe_stream(from_p, from_s, to_p, to_i)?;
        }

        // 3. Strateji Kurallarını Değerlendir
        for rule in &spec.rules {
            let rule_name = rule.name.as_deref().unwrap_or("Unnamed_Rule");
            println!("\x1b[93m\x1b[1m[JSON Kuralı Değerlendiriliyor]\x1b[0m {}", rule_name);

            let is_triggered = match &rule.when {
                Some(when_val) => Self::eval_condition(when_val),
                None => true,
            };

            if is_triggered {
                for action_val in &rule.then {
                    Self::execute_action(action_val, handler)?;
                }
            }
        }

        // 4. Doğrudan komutları çalıştır
        for cmd_val in &spec.commands {
            Self::execute_action(cmd_val, handler)?;
        }

        println!("\x1b[92m\x1b[1m✓ [JSON Engine]\x1b[0m STRATEJİ YÜRÜTMESİ TAMAMLANDI: {}\n", name);
        Ok(())
    }

    fn eval_condition(val: &JsonValue) -> bool {
        if let Some(obj) = val.as_object() {
            if let Some(arr) = obj.get("and").and_then(|v| v.as_array()) {
                return arr.iter().all(Self::eval_condition);
            }
            if let Some(arr) = obj.get("or").and_then(|v| v.as_array()) {
                return arr.iter().any(Self::eval_condition);
            }
            if let Some(sub) = obj.get("not") {
                return !Self::eval_condition(sub);
            }
        }
        true
    }

    fn execute_action<H: OrchestratorHandler>(val: &JsonValue, handler: &mut H) -> Result<(), String> {
        if let Some(action_str) = val.get("action").and_then(|v| v.as_str()) {
            match action_str {
                "buy" => {
                    let symbol = val.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTCUSDT");
                    let qty = val.get("qty").and_then(|v| v.as_f64()).unwrap_or(0.1);
                    let price = val.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let leverage = val.get("leverage").and_then(|v| v.as_f64()).unwrap_or(20.0);
                    handler.buy_order(symbol, qty, price, leverage)?;
                }
                "sell" => {
                    let symbol = val.get("symbol").and_then(|v| v.as_str()).unwrap_or("ETHUSDT");
                    let qty = val.get("qty").and_then(|v| v.as_f64()).unwrap_or(1.0);
                    let price = val.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let leverage = val.get("leverage").and_then(|v| v.as_f64()).unwrap_or(20.0);
                    handler.sell_order(symbol, qty, price, leverage)?;
                }
                "close" => {
                    let symbol = val.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTCUSDT");
                    handler.close_position(symbol)?;
                }
                "log" => {
                    let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    println!("\x1b[96m\x1b[1m[JSON Strategy LOG]\x1b[0m {}", msg);
                }
                "sql" => {
                    let query = val.get("query").and_then(|v| v.as_str()).unwrap_or("SELECT 1");
                    let res = handler.run_sql(query)?;
                    println!("\x1b[96m\x1b[1m[SQL Result]\x1b[0m\n{}", res);
                }
                _ => {}
            }
        }
        Ok(())
    }
}
