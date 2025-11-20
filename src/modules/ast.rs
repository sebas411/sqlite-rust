#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Ident(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    Star,
    Count,
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub columns: Vec<SelectItem>,
    pub table: String,
}
