#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Let,
    Plugin,
    Load,
    Start,
    Stop,
    PinCore,
    Pipe,
    When,
    Buy,
    Sell,
    Close,
    Log,
    Print,
    Sql,
    Fn,
    True,
    False,

    // Literals & Identifiers
    Ident(String),
    Number(f64),
    StringLit(String),

    // Symbols & Operators
    Assign,      // =
    Plus,        // +
    Minus,       // -
    Star,        // *
    Slash,       // /
    Gt,          // >
    Lt,          // <
    Gte,         // >=
    Lte,         // <=
    Eq,          // ==
    Neq,         // !=
    Arrow,       // ->
    Dot,         // .

    LParen,      // (
    RParen,      // )
    LBrace,      // {
    RBrace,      // }
    Comma,       // ,
    Colon,       // :
    Semicolon,   // ;

    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    chars: std::str::Chars<'a>,
    current_char: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer {
            input,
            chars: input.chars(),
            current_char: None,
        };
        lexer.advance();
        lexer
    }

    fn advance(&mut self) {
        self.current_char = self.chars.next();
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(c) = self.current_char {
            if c.is_whitespace() {
                self.advance();
            } else if c == '/' {
                let mut peek_chars = self.chars.clone();
                if peek_chars.next() == Some('/') {
                    // Line comment
                    self.advance();
                    self.advance();
                    while let Some(ch) = self.current_char {
                        if ch == '\n' {
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        let ch = match self.current_char {
            Some(c) => c,
            None => return Token::Eof,
        };

        if ch.is_alphabetic() || ch == '_' {
            return self.read_identifier();
        }

        if ch.is_ascii_digit() {
            return self.read_number();
        }

        if ch == '"' {
            return self.read_string();
        }

        match ch {
            '=' => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Token::Eq
                } else {
                    Token::Assign
                }
            }
            '>' => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Token::Gte
                } else {
                    Token::Gt
                }
            }
            '<' => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Token::Lte
                } else {
                    Token::Lt
                }
            }
            '!' => {
                self.advance();
                if self.current_char == Some('=') {
                    self.advance();
                    Token::Neq
                } else {
                    Token::Ident("!".to_string())
                }
            }
            '-' => {
                self.advance();
                if self.current_char == Some('>') {
                    self.advance();
                    Token::Arrow
                } else {
                    Token::Minus
                }
            }
            '+' => { self.advance(); Token::Plus }
            '*' => { self.advance(); Token::Star }
            '/' => { self.advance(); Token::Slash }
            '.' => { self.advance(); Token::Dot }
            '(' => { self.advance(); Token::LParen }
            ')' => { self.advance(); Token::RParen }
            '{' => { self.advance(); Token::LBrace }
            '}' => { self.advance(); Token::RBrace }
            ',' => { self.advance(); Token::Comma }
            ':' => { self.advance(); Token::Colon }
            ';' => { self.advance(); Token::Semicolon }
            _ => {
                self.advance();
                Token::Ident(ch.to_string())
            }
        }
    }

    fn read_identifier(&mut self) -> Token {
        let mut ident = String::new();
        while let Some(c) = self.current_char {
            if c.is_alphanumeric() || c == '_' {
                ident.push(c);
                self.advance();
            } else {
                break;
            }
        }

        match ident.as_str() {
            "let" => Token::Let,
            "plugin" => Token::Plugin,
            "load" => Token::Load,
            "start" => Token::Start,
            "stop" => Token::Stop,
            "pin_core" => Token::PinCore,
            "pipe" => Token::Pipe,
            "when" => Token::When,
            "buy" => Token::Buy,
            "sell" => Token::Sell,
            "close" => Token::Close,
            "log" => Token::Log,
            "print" => Token::Print,
            "sql" => Token::Sql,
            "fn" => Token::Fn,
            "true" => Token::True,
            "false" => Token::False,
            _ => Token::Ident(ident),
        }
    }

    fn read_number(&mut self) -> Token {
        let mut num_str = String::new();
        let mut has_dot = false;

        while let Some(c) = self.current_char {
            if c.is_ascii_digit() {
                num_str.push(c);
                self.advance();
            } else if c == '.' && !has_dot {
                // Check if next char is digit (so it's a float, not a method call `.`)
                let mut peek_chars = self.chars.clone();
                if peek_chars.next().map_or(false, |next_c| next_c.is_ascii_digit()) {
                    has_dot = true;
                    num_str.push('.');
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let val: f64 = num_str.parse().unwrap_or(0.0);
        Token::Number(val)
    }

    fn read_string(&mut self) -> Token {
        self.advance(); // Skip opening "
        let mut str_val = String::new();
        while let Some(c) = self.current_char {
            if c == '"' {
                self.advance();
                break;
            } else if c == '\\' {
                self.advance();
                if let Some(escaped) = self.current_char {
                    match escaped {
                        'n' => str_val.push('\n'),
                        't' => str_val.push('\t'),
                        'r' => str_val.push('\r'),
                        '\\' => str_val.push('\\'),
                        '"' => str_val.push('"'),
                        _ => str_val.push(escaped),
                    }
                    self.advance();
                }
            } else {
                str_val.push(c);
                self.advance();
            }
        }
        Token::StringLit(str_val)
    }
}
