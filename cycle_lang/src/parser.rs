use crate::ast::*;
use crate::lexer::{Lexer, Token};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next_token();
        let peek_token = lexer.next_token();
        Parser {
            lexer,
            current_token,
            peek_token,
        }
    }

    fn advance(&mut self) {
        self.current_token = std::mem::replace(&mut self.peek_token, self.lexer.next_token());
    }

    pub fn parse_program(&mut self) -> Result<Vec<Statement>, String> {
        let mut stmts = Vec::new();
        while self.current_token != Token::Eof {
            let stmt = self.parse_statement()?;
            stmts.push(stmt);
            if self.current_token == Token::Semicolon {
                self.advance();
            }
        }
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        match &self.current_token {
            Token::Pass => {
                self.advance();
                Ok(Statement::ExprStmt(Expr::Bool(true)))
            }
            Token::Import => self.parse_import_statement(),
            Token::Let => self.parse_let_statement(),
            Token::Pipe => self.parse_pipe_statement(),
            Token::If | Token::When => self.parse_if_statement(),
            Token::Buy => self.parse_buy_statement(),
            Token::Sell => self.parse_sell_statement(),
            Token::Close => self.parse_close_statement(),
            Token::Log => self.parse_log_statement(),
            Token::Print => self.parse_print_statement(),
            Token::Sql => self.parse_sql_statement(),
            Token::Def | Token::Fn => self.parse_fn_statement(),
            Token::Ident(_) => {
                if self.peek_token == Token::Assign {
                    self.parse_let_statement()
                } else {
                    self.parse_ident_statement()
                }
            }
            _ => {
                let expr = self.parse_expression(0)?;
                Ok(Statement::ExprStmt(expr))
            }
        }
    }

    fn parse_import_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'import'
        let path = match &self.current_token {
            Token::Ident(p) | Token::StringLit(p) => p.clone(),
            _ => return Err("'import' sonrasında eklenti adı bekleniyor".to_string()),
        };
        self.advance();

        let var_name = if self.current_token == Token::As {
            self.advance();
            match &self.current_token {
                Token::Ident(alias) => alias.clone(),
                _ => return Err("'as' sonrasında alias adı bekleniyor".to_string()),
            }
        } else {
            path.clone()
        };
        if self.current_token == Token::As { self.advance(); }

        Ok(Statement::PluginLoad { var_name, path })
    }

    fn parse_let_statement(&mut self) -> Result<Statement, String> {
        if self.current_token == Token::Let {
            self.advance(); // optional 'let'
        }

        let var_name = match &self.current_token {
            Token::Ident(name) => name.clone(),
            _ => return Err("Değişken adı bekleniyor".to_string()),
        };
        self.advance();

        if self.current_token != Token::Assign {
            return Err(format!("'{}' sonrasında '=' bekleniyor", var_name));
        }
        self.advance();

        // Check if `plugin.load(...)`
        if self.current_token == Token::Plugin {
            self.advance();
            if self.current_token == Token::Dot {
                self.advance();
                let is_load = match &self.current_token {
                    Token::Load => true,
                    Token::Ident(m) if m == "load" => true,
                    _ => false,
                };
                if is_load {
                    self.advance();
                    if self.current_token != Token::LParen {
                        return Err("'plugin.load' sonrasında '(' bekleniyor".to_string());
                    }
                    self.advance();
                    let path = match &self.current_token {
                        Token::StringLit(s) | Token::Ident(s) => s.clone(),
                        _ => return Err("'plugin.load' parametresi eklenti adı olmalı".to_string()),
                    };
                    self.advance();
                    if self.current_token == Token::RParen {
                        self.advance();
                    }
                    return Ok(Statement::PluginLoad { var_name, path });
                }
            }
        }

        let expr = self.parse_expression(0)?;
        Ok(Statement::Let { name: var_name, expr })
    }

    fn parse_ident_statement(&mut self) -> Result<Statement, String> {
        let var_name = match &self.current_token {
            Token::Ident(name) => name.clone(),
            _ => unreachable!(),
        };

        if self.peek_token == Token::Dot {
            self.advance(); // current = Ident, peek = Dot
            self.advance(); // current = Dot, peek = method Ident

            let method = match &self.current_token {
                Token::Ident(m) => m.clone(),
                Token::Start => "start".to_string(),
                Token::Stop => "stop".to_string(),
                Token::PinCore => "pin_core".to_string(),
                _ => return Err("Metot adı bekleniyor".to_string()),
            };
            self.advance();

            if self.current_token != Token::LParen {
                return Err(format!("'{}.{}' sonrasında '(' bekleniyor", var_name, method));
            }
            self.advance();

            match method.as_str() {
                "start" => {
                    if self.current_token == Token::RParen { self.advance(); }
                    Ok(Statement::PluginStart { var_name })
                }
                "stop" => {
                    if self.current_token == Token::RParen { self.advance(); }
                    Ok(Statement::PluginStop { var_name })
                }
                "pin_core" => {
                    let core = match &self.current_token {
                        Token::Number(n) => *n as usize,
                        _ => 0,
                    };
                    self.advance();
                    if self.current_token == Token::RParen { self.advance(); }
                    Ok(Statement::PluginPinCore { var_name, core })
                }
                _ => {
                    let mut args = Vec::new();
                    if self.current_token != Token::RParen {
                        args.push(self.parse_expression(0)?);
                        while self.current_token == Token::Comma {
                            self.advance();
                            args.push(self.parse_expression(0)?);
                        }
                    }
                    if self.current_token == Token::RParen { self.advance(); }
                    Ok(Statement::ExprStmt(Expr::PluginCall {
                        plugin: var_name,
                        method,
                        args,
                    }))
                }
            }
        } else {
            let expr = self.parse_expression(0)?;
            Ok(Statement::ExprStmt(expr))
        }
    }

    fn parse_pipe_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'pipe'
        if self.current_token == Token::LParen { self.advance(); }
        if let Token::Ident(_) = &self.current_token {
            if self.peek_token != Token::Dot && self.peek_token != Token::Comma {
                self.advance(); // optional pipe name
            }
        }
        if self.current_token == Token::LBrace { self.advance(); }

        let from_plugin = match &self.current_token {
            Token::Ident(p) => p.clone(),
            _ => return Err("Kaynak eklenti adı bekleniyor".to_string()),
        };
        self.advance();

        if self.current_token != Token::Dot {
            return Err("'.' bekleniyor".to_string());
        }
        self.advance();

        let from_stream = match &self.current_token {
            Token::Ident(s) | Token::StringLit(s) => s.clone(),
            _ => return Err("Kaynak akış adı bekleniyor".to_string()),
        };
        self.advance();

        if self.current_token == Token::Comma { self.advance(); }
        if self.current_token == Token::Arrow { self.advance(); }

        let to_plugin = match &self.current_token {
            Token::Ident(p) => p.clone(),
            _ => return Err("Hedef eklenti adı bekleniyor".to_string()),
        };
        self.advance();

        if self.current_token != Token::Dot {
            return Err("'.' bekleniyor".to_string());
        }
        self.advance();

        let to_inbox = match &self.current_token {
            Token::Ident(i) | Token::StringLit(i) => i.clone(),
            _ => return Err("Hedef inbox adı bekleniyor".to_string()),
        };
        self.advance();

        if self.current_token == Token::RParen { self.advance(); }
        if self.current_token == Token::Semicolon { self.advance(); }
        if self.current_token == Token::RBrace { self.advance(); }

        Ok(Statement::Pipe {
            from_plugin,
            from_stream,
            to_plugin,
            to_inbox,
        })
    }

    fn parse_if_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'if' or 'when'
        if self.current_token == Token::LParen {
            self.advance();
        }
        let condition = self.parse_expression(0)?;
        if self.current_token == Token::RParen {
            self.advance();
        }
        if self.current_token == Token::Colon {
            self.advance(); // skip ':' in Python `if cond:`
        }
        if self.current_token == Token::LBrace {
            self.advance(); // optional '{'
        }

        let mut body = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof && self.current_token != Token::Else && self.current_token != Token::Elif {
            let stmt = self.parse_statement()?;
            body.push(stmt);
            if self.current_token == Token::Semicolon {
                self.advance();
            }
        }
        if self.current_token == Token::RBrace {
            self.advance();
        }

        Ok(Statement::When { condition, body })
    }

    fn parse_buy_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'buy'
        if self.current_token == Token::LParen { self.advance(); }
        let symbol = self.parse_expression(0)?;
        if self.current_token == Token::Comma { self.advance(); }

        let qty = self.parse_named_or_expr("qty")?;
        if self.current_token == Token::Comma { self.advance(); }

        let price = self.parse_named_or_expr("price")?;
        let leverage = if self.current_token == Token::Comma {
            self.advance();
            self.parse_named_or_expr("leverage")?
        } else {
            Expr::Number(20.0)
        };

        if self.current_token == Token::RParen { self.advance(); }
        Ok(Statement::Buy { symbol, qty, price, leverage })
    }

    fn parse_sell_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'sell'
        if self.current_token == Token::LParen { self.advance(); }
        let symbol = self.parse_expression(0)?;
        if self.current_token == Token::Comma { self.advance(); }

        let qty = self.parse_named_or_expr("qty")?;
        if self.current_token == Token::Comma { self.advance(); }

        let price = self.parse_named_or_expr("price")?;
        let leverage = if self.current_token == Token::Comma {
            self.advance();
            self.parse_named_or_expr("leverage")?
        } else {
            Expr::Number(20.0)
        };

        if self.current_token == Token::RParen { self.advance(); }
        Ok(Statement::Sell { symbol, qty, price, leverage })
    }

    fn parse_close_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'close'
        if self.current_token == Token::LParen { self.advance(); }
        let symbol = self.parse_expression(0)?;
        if self.current_token == Token::RParen { self.advance(); }
        Ok(Statement::Close { symbol })
    }

    fn parse_log_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'log'
        if self.current_token == Token::LParen { self.advance(); }
        let message = self.parse_expression(0)?;
        if self.current_token == Token::RParen { self.advance(); }
        Ok(Statement::Log { message })
    }

    fn parse_print_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'print'
        if self.current_token == Token::LParen { self.advance(); }
        let expr = self.parse_expression(0)?;
        if self.current_token == Token::RParen { self.advance(); }
        Ok(Statement::Print { expr })
    }

    fn parse_sql_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'sql'
        if self.current_token == Token::LParen { self.advance(); }
        let query = self.parse_expression(0)?;
        if self.current_token == Token::RParen { self.advance(); }
        Ok(Statement::Sql { query })
    }

    fn parse_fn_statement(&mut self) -> Result<Statement, String> {
        self.advance(); // skip 'def' or 'fn'
        let name = match &self.current_token {
            Token::Ident(n) => n.clone(),
            _ => return Err("Fonksiyon adı bekleniyor".to_string()),
        };
        self.advance();

        if self.current_token != Token::LParen {
            return Err("'(' bekleniyor".to_string());
        }
        self.advance();

        let mut params = Vec::new();
        while self.current_token != Token::RParen && self.current_token != Token::Eof {
            if let Token::Ident(p) = &self.current_token {
                params.push(p.clone());
            }
            self.advance();
            if self.current_token == Token::Comma {
                self.advance();
            }
        }
        if self.current_token == Token::RParen { self.advance(); }

        if self.current_token == Token::Colon {
            self.advance(); // skip ':' in Python `def foo():`
        }
        if self.current_token == Token::LBrace {
            self.advance(); // optional '{'
        }

        let mut body = Vec::new();
        while self.current_token != Token::RBrace && self.current_token != Token::Eof {
            let stmt = self.parse_statement()?;
            body.push(stmt);
            if self.current_token == Token::Semicolon { self.advance(); }
        }
        if self.current_token == Token::RBrace { self.advance(); }

        Ok(Statement::FnDef { name, params, body })
    }

    fn parse_named_or_expr(&mut self, _name: &str) -> Result<Expr, String> {
        if let Token::Ident(_) = &self.current_token {
            if self.peek_token == Token::Assign || self.peek_token == Token::Colon {
                self.advance(); // skip ident
                self.advance(); // skip '=' or ':'
            }
        }
        self.parse_expression(0)
    }

    fn parse_expression(&mut self, precedence: u8) -> Result<Expr, String> {
        let mut left = match &self.current_token {
            Token::Number(n) => {
                let val = *n;
                self.advance();
                Expr::Number(val)
            }
            Token::StringLit(s) => {
                let val = s.clone();
                self.advance();
                Expr::StringLit(val)
            }
            Token::True => {
                self.advance();
                Expr::Bool(true)
            }
            Token::False => {
                self.advance();
                Expr::Bool(false)
            }
            Token::Ident(name) => {
                let id = name.clone();
                self.advance();
                if self.current_token == Token::Dot {
                    self.advance();
                    if let Token::Ident(method) = &self.current_token {
                        let m = method.clone();
                        self.advance();
                        if self.current_token == Token::LParen {
                            self.advance();
                            let mut args = Vec::new();
                            if self.current_token != Token::RParen {
                                args.push(self.parse_expression(0)?);
                                while self.current_token == Token::Comma {
                                    self.advance();
                                    args.push(self.parse_expression(0)?);
                                }
                            }
                            if self.current_token == Token::RParen { self.advance(); }
                            Expr::PluginCall { plugin: id, method: m, args }
                        } else {
                            Expr::PluginCall { plugin: id, method: m, args: vec![] }
                        }
                    } else {
                        Expr::Var(id)
                    }
                } else if self.current_token == Token::LParen {
                    self.advance();
                    let mut args = Vec::new();
                    if self.current_token != Token::RParen {
                        args.push(self.parse_expression(0)?);
                        while self.current_token == Token::Comma {
                            self.advance();
                            args.push(self.parse_expression(0)?);
                        }
                    }
                    if self.current_token == Token::RParen { self.advance(); }
                    Expr::FnCall { name: id, args }
                } else {
                    Expr::Var(id)
                }
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expression(0)?;
                if self.current_token == Token::RParen { self.advance(); }
                expr
            }
            _ => return Err(format!("İfade ayrıştırılamadı: {:?}", self.current_token)),
        };

        while self.current_token != Token::Eof && precedence < self.get_precedence(&self.current_token) {
            let op = match &self.current_token {
                Token::Plus => "+",
                Token::Minus => "-",
                Token::Star => "*",
                Token::Slash => "/",
                Token::Gt => ">",
                Token::Lt => "<",
                Token::Gte => ">=",
                Token::Lte => "<=",
                Token::Eq => "==",
                Token::Neq => "!=",
                Token::And => "and",
                Token::Or => "or",
                _ => break,
            }.to_string();

            let cur_prec = self.get_precedence(&self.current_token);
            self.advance();
            let right = self.parse_expression(cur_prec)?;
            left = Expr::BinOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn get_precedence(&self, token: &Token) -> u8 {
        match token {
            Token::Or => 1,
            Token::And => 2,
            Token::Eq | Token::Neq => 3,
            Token::Gt | Token::Lt | Token::Gte | Token::Lte => 4,
            Token::Plus | Token::Minus => 5,
            Token::Star | Token::Slash => 6,
            _ => 0,
        }
    }
}
