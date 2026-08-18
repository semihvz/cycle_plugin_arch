use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonPluginSpec {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub var: String,
    pub path: String,
    #[serde(default)]
    pub core: Option<usize>,
}

impl JsonPluginSpec {
    pub fn get_name(&self) -> &str {
        if !self.id.is_empty() {
            &self.id
        } else if !self.var.is_empty() {
            &self.var
        } else {
            &self.path
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonPipeSpec {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRuleSpec {
    pub name: Option<String>,
    pub when: Option<serde_json::Value>,
    #[serde(default)]
    pub then: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonStrategySpec {
    #[serde(default)]
    pub strategy_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub plugins: Vec<JsonPluginSpec>,
    #[serde(default)]
    pub pipes: Vec<JsonPipeSpec>,
    #[serde(default)]
    pub rules: Vec<JsonRuleSpec>,
    #[serde(default)]
    pub commands: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Number(n) => {
                if n.fract() == 0.0 {
                    write!(f, "{:.0}", n)
                } else {
                    write!(f, "{:.4}", n)
                }
            }
            Value::String(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Nil => write!(f, "nil"),
        }
    }
}
