use anyhow::{Ok, Result, anyhow, bail};
use std::fs::File;
use std::io::prelude::*;

use crate::modules::{ast::{Expr, SelectItem}, sql_parser::sql_parser, table::{Column, Table}};
mod modules;

fn get_column_size(ctype: i64) -> usize {
    if ctype < 12 {
        ctype as usize
    } else if ctype % 2 == 0 {
        (ctype as usize - 12)/2
    } else {
        (ctype as usize - 13)/2
    }
}

fn get_varint(data: &[u8], current_offset: &mut usize) -> i64 {
    let mut out: i64 = 0;
    let mut byte_num = 0;
    for byte in &data[*current_offset..] {
        byte_num += 1;
        if *byte > 127 && byte_num < 9 {
            let tmp = *byte & (255-128);
            out <<= 7;
            out |= tmp as i64;
        } else {
            if byte_num == 9 {
                out <<= 1;
            }
            out <<= 7;
            out |= *byte as i64;
            break;
        }
    }
    *current_offset += byte_num;
    out
}

fn get_table_info(buffer: &[u8]) -> Vec<Table> {
    let table_num = u16::from_be_bytes([buffer[100+3], buffer[100+4]]);
    let mut tables = vec![];
    for i in 0..table_num as usize {
        let mut current_offset = u16::from_be_bytes([buffer[100+8+2*i], buffer[100+8+2*i+1]]) as usize;
        get_varint(&buffer, &mut current_offset); // size of record
        get_varint(&buffer, &mut current_offset); // the rowid
        get_varint(&buffer, &mut current_offset); // record header size
        let mut record_offset = 0;
        record_offset += (get_varint(&buffer, &mut current_offset)-13)/2; // sqlite_schema.type
        record_offset += (get_varint(&buffer, &mut current_offset)-13)/2; // sqlite_schema.name
        let tbl_name_size = (get_varint(&buffer, &mut current_offset)as usize-13)/2; // sqlite_schema.tbl_name
        get_varint(&buffer, &mut current_offset); // sqlite_schema.rootpage
        let sql_size = (get_varint(&buffer, &mut current_offset)as usize-13)/2; // sqlite_schema.sql
        current_offset += record_offset as usize;
        let tbl_name = buffer[current_offset..current_offset+tbl_name_size].to_vec();
        let tbl_name_string = String::from_utf8(tbl_name).expect("table name is not a string");
        if tbl_name_string == "sqlite_sequence" {
            continue;
        }
        current_offset += tbl_name_size;
        let rootpage = buffer[current_offset];
        let sql = buffer[current_offset+1..current_offset+1+sql_size].to_vec();
        let sql_string = String::from_utf8(sql).expect("table sql is not a string");
        let parameters = sql_string.split('(').nth(1).unwrap().strip_suffix(')').unwrap();

        let mut columns = vec![];
        for parameter in parameters.split(',') {
            let parameter = parameter.trim().split(" ").collect::<Vec<_>>();
            let parameter_name = parameter[0];
            let parameter_type = parameter[1];
            let column = Column::new(parameter_name, parameter_type);
            columns.push(column);
        }

        let table = Table::new(&tbl_name_string, rootpage, columns);

        tables.push(table);
    }
    tables
}

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
                    SelectItem::Expr(item) => {
                        let Expr::Ident(item) = item;
                        let cnum = available_columns.iter().position(|c| &c.name == item).ok_or(anyhow!("Column not on table"))?;
                        print_columns.push(cnum);
                    },
                    SelectItem::Star => ()
                }
            }

            // read each cell
            for i in 0..cell_num as usize {
                let mut current_offset = u16::from_be_bytes([buffer[8+2*i], buffer[8+2*i+1]]) as usize;
                let _record_size = get_varint(&buffer, &mut current_offset); // size of record
                get_varint(&buffer, &mut current_offset); // the rowid

                // read cell header
                let record_header_start = current_offset;
                let record_header_size = get_varint(&buffer, &mut current_offset); // record header size
                let mut column_sizes = vec![];
                for _ in 0..available_columns.len() {
                    let csize = get_varint(&buffer, &mut current_offset);
                    column_sizes.push(csize);
                }
                if record_header_start + record_header_size as usize != current_offset {
                    bail!("Did not get to the end of record header! Expected: {}, current: {}", record_header_start + record_header_size as usize, current_offset)
                }

                // read record content (loop for each column)
                for j in 0..available_columns.len() {
                    let size = get_column_size(column_sizes[j]);
                    let content_bytes = buffer[current_offset..current_offset+size].to_vec();
                    current_offset += size;
                    if print_columns.contains(&j) {
                        let content = String::from_utf8(content_bytes)?;
                        println!("{}", content);
                    }
                }
            }
        },
    }

    Ok(())
}
