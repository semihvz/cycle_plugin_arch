use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    Let {
        name: String,
        expr: Expr,
    },
    PluginLoad {
        var_name: String,
        path: String,
    },
    PluginStart {
        var_name: String,
    },
    PluginStop {
        var_name: String,
    },
    PluginPinCore {
        var_name: String,
        core: usize,
    },
    Pipe {
        from_plugin: String,
        from_stream: String,
        to_plugin: String,
        to_inbox: String,
    },
    When {
        condition: Expr,
        body: Vec<Statement>,
    },
    Buy {
        symbol: Expr,
        qty: Expr,
        price: Expr,
        leverage: Expr,
    },
    Sell {
        symbol: Expr,
        qty: Expr,
        price: Expr,
        leverage: Expr,
    },
    Close {
        symbol: Expr,
    },
    Log {
        message: Expr,
    },
    Print {
        expr: Expr,
    },
    Sql {
        query: Expr,
    },
    Sleep {
        seconds: Expr,
    },
    FnDef {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    ExprStmt(Expr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Number(f64),
    StringLit(String),
    Bool(bool),
    Var(String),
    BinOp {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
    },
    PluginCall {
        plugin: String,
        method: String,
        args: Vec<Expr>,
    },
    FnCall {
        name: String,
        args: Vec<Expr>,
    },
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
