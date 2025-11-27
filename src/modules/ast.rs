use std::fmt;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Equality {
        column: Literal,
        condition: Literal,
    },
}

impl Expr {
    pub fn get_equality(&self) -> Result<(Literal, Literal)> {
        if let Self::Equality { column, condition } = self {
            return Ok((column.clone(), condition.clone()))
        }
        Err(anyhow!("Expr isnt an equality."))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Literal {
    Ident(String),
    StringLiteral(String),
    NumberLiteral(f64),
    Null
}

impl Literal {
    pub fn get_ident(&self) -> Result<String> {
        if let Self::Ident(s) = self {
            return Ok(s.clone())
        }
        Err(anyhow!("Literal isnt an ident"))
    }
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
