use anyhow::{Ok, Result, anyhow, bail};
use std::fs::File;
use std::io::prelude::*;

use crate::modules::{ast::{Literal, SelectItem}, helpers::{get_table_info, read_cell}, sql_parser::sql_parser};
mod modules;

fn main() -> Result<()> {
    // Parse arguments
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        0 | 1 => bail!("Missing <database path> and <command>"),
        2 => bail!("Missing <command>"),
        _ => {}
    }

    let mut file = File::open(&args[1])?;
    let mut header = [0; 100];
    file.read_exact(&mut header)?;

    // The page size is stored at the 16th byte offset, using 2 bytes in big-endian order
    let page_size = u16::from_be_bytes([header[16], header[17]]);

    let mut buffer = Vec::new();
    buffer.resize(page_size as usize, 0u8);

    file.read_exact(&mut buffer[100..])?; // read the rest of the first page (sqlite schemas)

    let table_num = u16::from_be_bytes([buffer[100+3], buffer[100+4]]);

    let tables = get_table_info(&buffer);

    // Parse command and act accordingly
    let command = &args[2];
    match command.as_str() {
        ".dbinfo" => {

            println!("database page size: {}", page_size);
            println!("number of tables: {}", table_num);
        },
        ".tables" => {
            
            
            for table in tables {
                print!("{} ", table.name)
            }
            println!()
        },
        query => {
            let select_stmt = sql_parser::statement(query)?;
            let table_name = select_stmt.table;
            let my_table = tables.into_iter().find(|table| table.name == table_name).ok_or(anyhow!("Table not found"))?;
            if my_table.rootpage <= 1 {
                bail!("table not found")
            }
            for _ in 1..my_table.rootpage {
                file.read_exact(&mut buffer)?; // read page
            }
            let available_columns = my_table.columns;
            let mut print_columns = vec![];
            let cell_num = u16::from_be_bytes([buffer[3], buffer[4]]);

            // Check SELECT columns
            for column in &select_stmt.columns {
                match column {
                    SelectItem::Count => {
                        println!("{}", cell_num);
                        return Ok(());
                    },
                    SelectItem::Literal(item) => {
                        if let Literal::Ident(item) = item {
                            let cnum = available_columns.iter().position(|c| &c.name == item).ok_or(anyhow!("Column not on table"))?;
                            print_columns.push(cnum);
                        }
                    },
                    SelectItem::Star => {
                        for i in 0..available_columns.len() {
                            print_columns.push(i);
                        }
                    }
                }
            }
            read_cell(&file, my_table.rootpage as u32, page_size as usize, &select_stmt.where_expr, &available_columns, &print_columns)?
        },
    }

    Ok(())
}
