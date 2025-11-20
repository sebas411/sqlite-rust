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
