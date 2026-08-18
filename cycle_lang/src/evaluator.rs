use crate::ast::*;
use std::collections::HashMap;

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

pub struct Evaluator {
    env: HashMap<String, Value>,
    functions: HashMap<String, (Vec<String>, Vec<Statement>)>,
}

impl Evaluator {
    pub fn new() -> Self {
        Evaluator {
            env: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    pub fn eval_program<H: OrchestratorHandler>(
        &mut self,
        stmts: &[Statement],
        handler: &mut H,
    ) -> Result<(), String> {
        for stmt in stmts {
            self.eval_statement(stmt, handler)?;
        }
        Ok(())
    }

    pub fn eval_statement<H: OrchestratorHandler>(
        &mut self,
        stmt: &Statement,
        handler: &mut H,
    ) -> Result<(), String> {
        match stmt {
            Statement::Let { name, expr } => {
                let val = self.eval_expr(expr, handler)?;
                self.env.insert(name.clone(), val);
            }
            Statement::PluginLoad { var_name, path } => {
                handler.load_plugin(var_name, path)?;
                self.env.insert(var_name.clone(), Value::String(path.clone()));
            }
            Statement::PluginStart { var_name } => {
                handler.start_plugin(var_name)?;
            }
            Statement::PluginStop { var_name } => {
                handler.stop_plugin(var_name)?;
            }
            Statement::PluginPinCore { var_name, core } => {
                handler.pin_core(var_name, *core)?;
            }
            Statement::Pipe {
                from_plugin,
                from_stream,
                to_plugin,
                to_inbox,
            } => {
                handler.pipe_stream(from_plugin, from_stream, to_plugin, to_inbox)?;
            }
            Statement::When { condition, body } => {
                let cond_val = self.eval_expr(condition, handler)?;
                if self.is_truthy(&cond_val) {
                    for body_stmt in body {
                        self.eval_statement(body_stmt, handler)?;
                    }
                }
            }
            Statement::Buy {
                symbol,
                qty,
                price,
                leverage,
            } => {
                let sym_str = self.eval_expr(symbol, handler)?.to_string();
                let q = self.eval_number(qty, handler)?;
                let p = self.eval_number(price, handler)?;
                let lev = self.eval_number(leverage, handler)?;
                handler.buy_order(&sym_str, q, p, lev)?;
            }
            Statement::Sell {
                symbol,
                qty,
                price,
                leverage,
            } => {
                let sym_str = self.eval_expr(symbol, handler)?.to_string();
                let q = self.eval_number(qty, handler)?;
                let p = self.eval_number(price, handler)?;
                let lev = self.eval_number(leverage, handler)?;
                handler.sell_order(&sym_str, q, p, lev)?;
            }
            Statement::Close { symbol } => {
                let sym_str = self.eval_expr(symbol, handler)?.to_string();
                handler.close_position(&sym_str)?;
            }
            Statement::Log { message } => {
                let msg_val = self.eval_expr(message, handler)?;
                println!("\x1b[96m\x1b[1m[CycleLang LOG]\x1b[0m {}", msg_val);
            }
            Statement::Print { expr } => {
                let val = self.eval_expr(expr, handler)?;
                println!("{}", val);
            }
            Statement::Sql { query } => {
                let q_str = self.eval_expr(query, handler)?.to_string();
                let res = handler.run_sql(&q_str)?;
                println!("\x1b[96m\x1b[1m[SQL Result]\x1b[0m\n{}", res);
            }
            Statement::Sleep { seconds } => {
                let secs = self.eval_number(seconds, handler)?;
                std::thread::sleep(std::time::Duration::from_secs_f64(secs));
            }
            Statement::FnDef { name, params, body } => {
                self.functions
                    .insert(name.clone(), (params.clone(), body.clone()));
            }
            Statement::ExprStmt(expr) => {
                let _ = self.eval_expr(expr, handler)?;
            }
        }
        Ok(())
    }

    fn eval_number<H: OrchestratorHandler>(
        &mut self,
        expr: &Expr,
        handler: &mut H,
    ) -> Result<f64, String> {
        let val = self.eval_expr(expr, handler)?;
        match val {
            Value::Number(n) => Ok(n),
            _ => Err(format!("Sayısal değer bekleniyor, '{}' bulundu", val)),
        }
    }

    fn is_truthy(&self, val: &Value) -> bool {
        match val {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Nil => false,
        }
    }

    pub fn eval_expr<H: OrchestratorHandler>(
        &mut self,
        expr: &Expr,
        handler: &mut H,
    ) -> Result<Value, String> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::StringLit(s) => Ok(Value::String(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Var(name) => {
                if let Some(val) = self.env.get(name) {
                    Ok(val.clone())
                } else {
                    Ok(Value::String(name.clone()))
                }
            }
            Expr::BinOp { left, op, right } => {
                let l_val = self.eval_expr(left, handler)?;
                let r_val = self.eval_expr(right, handler)?;
                self.eval_binary_op(&l_val, op, &r_val)
            }
            Expr::PluginCall { plugin, method, args } => {
                let mut evaled_args = Vec::new();
                for a in args {
                    evaled_args.push(self.eval_expr(a, handler)?);
                }
                handler.call_plugin(plugin, method, &evaled_args)
            }
            Expr::FnCall { name, args } => {
                if let Some((params, body)) = self.functions.get(name).cloned() {
                    if params.len() != args.len() {
                        return Err(format!("Fonksiyon '{}' için {} parametre bekleniyor", name, params.len()));
                    }

                    let old_env = self.env.clone();
                    for (p, arg_expr) in params.iter().zip(args.iter()) {
                        let val = self.eval_expr(arg_expr, handler)?;
                        self.env.insert(p.clone(), val);
                    }

                    for stmt in &body {
                        self.eval_statement(stmt, handler)?;
                    }

                    self.env = old_env;
                    Ok(Value::Nil)
                } else {
                    Err(format!("Tanımlanmamış fonksiyon: '{}'", name))
                }
            }
        }
    }

    fn eval_binary_op(&self, left: &Value, op: &str, right: &Value) -> Result<Value, String> {
        match (left, right) {
            (Value::Number(l), Value::Number(r)) => match op {
                "+" => Ok(Value::Number(l + r)),
                "-" => Ok(Value::Number(l - r)),
                "*" => Ok(Value::Number(l * r)),
                "/" => {
                    if *r != 0.0 {
                        Ok(Value::Number(l / r))
                    } else {
                        Err("Sıfıra bölme hatası".to_string())
                    }
                }
                ">" => Ok(Value::Bool(l > r)),
                "<" => Ok(Value::Bool(l < r)),
                ">=" => Ok(Value::Bool(l >= r)),
                "<=" => Ok(Value::Bool(l <= r)),
                "==" => Ok(Value::Bool(l == r)),
                "!=" => Ok(Value::Bool(l != r)),
                _ => Err(format!("Bilinmeyen sayısal operatör: {}", op)),
            },
            (Value::String(l), Value::String(r)) => match op {
                "+" => Ok(Value::String(format!("{}{}", l, r))),
                "==" => Ok(Value::Bool(l == r)),
                "!=" => Ok(Value::Bool(l != r)),
                _ => Err(format!("Bilinmeyen metin operatörü: {}", op)),
            },
            (Value::String(l), Value::Number(r)) if op == "+" => {
                if r.fract() == 0.0 {
                    Ok(Value::String(format!("{}{:.0}", l, r)))
                } else {
                    Ok(Value::String(format!("{}{:.4}", l, r)))
                }
            }
            (Value::Number(l), Value::String(r)) if op == "+" => {
                if l.fract() == 0.0 {
                    Ok(Value::String(format!("{:.0}{}", l, r)))
                } else {
                    Ok(Value::String(format!("{:.4}{}", l, r)))
                }
            }
            _ => Err(format!("Desteklenmeyen veri tipleri ile işlem: {:?} {} {:?}", left, op, right)),
        }
    }
}
