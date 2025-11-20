use crate::modules::ast::{Expr, SelectItem, SelectStatement};

peg::parser! {
    pub grammar sql_parser() for str {
        // ---- Entry point ----
        pub rule statement() -> SelectStatement
            = _ s:select_stmt() _ ";"? _ { s }

        // ---- SELECT ----
        rule select_stmt() -> SelectStatement
            = kw_select() _ cols:select_list() _ kw_from() _ table:ident() {
                SelectStatement {
                    columns: cols,
                    table,
                }
            }

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
            = e:expr() { SelectItem::Expr(e) }

        // ---- Expressions (very minimal for now) ----
        rule expr() -> Expr
            = id:ident() { Expr::Ident(id) }

        // ---- Identifiers and keywords ----

        /// SQL identifier: starts with letter or '_', then letters/digits/'_'
        rule ident() -> String
            = id:quiet!{
                $(['a'..='z' | 'A'..='Z' | '_']['a'..='z' | 'A'..='Z' | '0'..='9' | '_']*)
            } { 
                id.to_string()
            }
            / expected!("identifier")

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

        // ---- Whitespace & comments ----
        rule _()
            = quiet!{ [' ' | '\t' | '\n' | '\r']* }
    }
}