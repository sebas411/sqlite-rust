use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Equality {
        column: Literal,
        condition: Literal,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Ident(String),
    StringLiteral(String),
    NumberLiteral(f64),
    Null
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ident(s) => write!(f, "{}", s),
            Self::StringLiteral(s) => write!(f, "{}", s),
            Self::NumberLiteral(n) => write!(f, "{}", n),
            Self::Null => write!(f, "null"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    Star,
    Count,
    Literal(Literal),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub columns: Vec<SelectItem>,
    pub table: String,
    pub where_expr: Option<Expr>
}
