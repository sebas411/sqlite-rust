use crate::modules::ast::{Expr, Literal, SelectItem, SelectStatement};

peg::parser! {
    pub grammar sql_parser() for str {
        // ---- Entry point ----
        pub rule statement() -> SelectStatement
            = _ s:select_stmt() _ ";"? _ { s }

        // ---- SELECT ----
        rule select_stmt() -> SelectStatement
            = kw_select() _ cols:select_list() _ kw_from() _ table:ident() _ where_clause:where_clause()? {
                SelectStatement {
                    columns: cols,
                    table,
                    where_expr: where_clause
                }
            }

        rule where_clause() -> Expr
            = kw_where() _ e:expr() {e}

        rule select_list() -> Vec<SelectItem>
            = "*" { vec![SelectItem::Star] }
            / kw_count() "(*)" { vec![SelectItem::Count] }
            / head:select_item() tail:(_ "," _ item:select_item() {item})* {
                let mut v = Vec::new();
                v.push(head);
                for item in tail {
                    v.push(item);
                }
                v
            }

        rule select_item() -> SelectItem
            = l:literal() { SelectItem::Literal(l) }

        // ---- Expressions ----
        rule expr() -> Expr
            = col:literal() _ "=" _ con:literal() { Expr::Equality { column: col, condition: con } }
            / l:literal() { Expr::Literal(l) }

        rule literal() -> Literal
            = id:ident() { Literal::Ident(id) }
            / s:string_literal() { Literal::StringLiteral(s) }
            / n:number_literal() { Literal::NumberLiteral(n) }

        // ---- Identifiers and keywords ----

        /// SQL identifier: starts with letter or '_', then letters/digits/'_'
        rule ident() -> String
            = id:quiet!{
                $(['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*)
            } { 
                id.to_string()
            }
            / expected!("identifier")

        // Strings (surrounded by single quotes)
        rule string_literal() -> String
            = "'" s:$([^ '\'']*) "'" { s.to_string() }

        // Numbers
        rule number_literal() -> f64
            = n:$(['0'..='9']+ ("." ['0'..='9']+)? ) { n.parse().unwrap() }

        /// Case-insensitive SELECT
        rule kw_select()
            = quiet!{ 
                "SELECT" / "select" / "Select" // simple way; later you can make a helper
            }
            / expected!("SELECT")

        /// Case-insensitive FROM
        rule kw_from()
            = quiet!{
                "FROM" / "from" / "From"
            }
            / expected!("FROM")

        /// Case-insensitive COUNT
        rule kw_count()
            = quiet!{
                "COUNT" / "count" / "Count"
            }
            / expected!("COUNT")

        rule kw_where()
            = quiet!{
                "WHERE" / "where" / "Where"
            }
            / expected!("WHERE")

        // ---- Whitespace & comments ----
        rule _()
            = quiet!{ [' ' | '\t' | '\n' | '\r']* }
    }
}