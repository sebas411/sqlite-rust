use std::{fs::File, os::unix::fs::FileExt};

use anyhow::{Result, anyhow, bail};

use crate::modules::{ast::{Expr, Literal}, table::{Column, Table}};

fn get_column_size(ctype: i64) -> usize {
    if ctype < 12 {
        if ctype == 5 {
            6
        } else if ctype == 6 || ctype == 7 {
            8
        } else if ctype == 8 || ctype == 9 {
            0
        } else {
            ctype as usize
        }
    } else if ctype % 2 == 0 {
        (ctype as usize - 12)/2
    } else {
        (ctype as usize - 13)/2
    }
}

fn get_column_type(ctype: i64) -> u8 {
    if ctype < 12 {
        ctype as u8
    } else if ctype % 2 == 0 {
        12
    } else {
        13
    }
}

fn get_u64_from_size_n(buff: &[u8], n: usize) -> u64 {
    let mut my_num = 0;
    for i in 0..n {
        my_num <<= 8;
        my_num |= buff[i] as u64;
    }
    my_num
}

fn get_num_size_n(buff: &[u8], n: usize, is_float: bool) -> f64 {
    if is_float {
        let mut hull = 0u64;
        for i in 0..n {
            let my_byte = buff[i];
            hull <<= 8;
            hull |= my_byte as u64;
        }
        f64::from_bits(hull)
    } else {
        if n == 1 {
            buff[0] as f64
        } else if n == 2 {
            let my_num = i16::from_be_bytes([buff[0], buff[1]]);
            my_num as f64
        } else if n == 4 {
            let my_num = i32::from_be_bytes([buff[0], buff[1], buff[2], buff[3]]);
            my_num as f64
        } else if n == 8 {
            let my_num = i64::from_be_bytes([buff[0], buff[1], buff[2], buff[3], buff[4], buff[5], buff[6], buff[7]]);
            my_num as f64
        } else {
            0.0
        }
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

pub fn get_table_info(buffer: &[u8]) -> (Vec<Table>, Vec<Table>) {
    let table_num = u16::from_be_bytes([buffer[100+3], buffer[100+4]]);
    let mut tables = vec![];
    let mut indices = vec![];
    for i in 0..table_num as usize {
        let mut current_offset = u16::from_be_bytes([buffer[100+8+2*i], buffer[100+8+2*i+1]]) as usize;
        get_varint(&buffer, &mut current_offset); // size of record
        get_varint(&buffer, &mut current_offset); // the rowid
        get_varint(&buffer, &mut current_offset); // record header size
        let schema_type_size = (get_varint(&buffer, &mut current_offset)as usize-13)/2; // sqlite_schema.type
        let schema_name_size = (get_varint(&buffer, &mut current_offset)as usize-13)/2; // sqlite_schema.name
        let tbl_name_size = (get_varint(&buffer, &mut current_offset)as usize-13)/2; // sqlite_schema.tbl_name
        let rootpage_size = get_varint(&buffer, &mut current_offset); // sqlite_schema.rootpage
        let sql_size = (get_varint(&buffer, &mut current_offset)as usize-13)/2; // sqlite_schema.sql

        // start record payload
        let schema_type = String::from_utf8(buffer[current_offset..current_offset+schema_type_size].to_vec()).unwrap();
        current_offset += schema_type_size + schema_name_size;
        let tbl_name = buffer[current_offset..current_offset+tbl_name_size].to_vec();
        let tbl_name_string = String::from_utf8(tbl_name).expect("table name is not a string");
        if tbl_name_string == "sqlite_sequence" {
            continue;
        }
        current_offset += tbl_name_size;
        let rootpage = get_u64_from_size_n(&buffer[current_offset..], rootpage_size as usize) as u32;
        current_offset += rootpage_size as usize;
        let sql = buffer[current_offset..current_offset+sql_size].to_vec();
        let sql_string = String::from_utf8(sql).expect("table sql is not a string");
        let parameters = sql_string.split('(').nth(1).unwrap().strip_suffix(')').unwrap();

        let mut columns = vec![];
        for parameter in parameters.split(',') {
            let parameter = parameter.trim().split(" ").collect::<Vec<_>>();
            let parameter_name = parameter[0];
            let parameter_type;
            if schema_type == "table" {
                parameter_type = parameter[1];
            } else {
                parameter_type = "";
            }
            let column = Column::new(parameter_name, parameter_type);
            columns.push(column);
        }

        let table = Table::new(&tbl_name_string, rootpage, columns);

        if schema_type == "table" {
            tables.push(table);
        } else if schema_type == "index" {
            indices.push(table);
        } else {
            panic!("Unrecognized schema type");
        }
    }
    (tables, indices)
}

pub fn read_index(file: &File, page_num: u32, page_size: usize, where_expr: &Expr, available_columns: &Vec<Column>, print_columns: &Vec<usize>, indexed_columns: &Vec<Column>, table_page_num: u32) -> Result<()> {
    let mut buffer  = Vec::new();
    buffer.resize(page_size, 0u8);
    let page_offset = (page_size*(page_num as usize - 1)) as u64;
    file.read_exact_at(&mut buffer, page_offset)?; // read page
    
    let page_type = buffer[0];
    let cell_num = u16::from_be_bytes([buffer[3], buffer[4]]);

    let where_col_num = indexed_columns.iter().position(|c| c.name == where_expr.get_equality().unwrap().0.get_ident().unwrap()).unwrap();

    // leaf index
    if page_type == 10 {
        for i in 0..cell_num as usize {
            let mut current_offset = u16::from_be_bytes([buffer[8+2*i], buffer[8+2*i+1]]) as usize;
            get_varint(&buffer, &mut current_offset); // payload size
            // start payload
            get_varint(&buffer, &mut current_offset); // header size
            // start header
            let mut column_sizes = vec![];
            for _ in 0..indexed_columns.len() {
                let csize = get_varint(&buffer, &mut current_offset);
                column_sizes.push(csize);
            }
            let rowid_size = get_varint(&buffer, &mut current_offset);
            column_sizes.push(rowid_size);
            let mut index_cols = vec![];
            for j in 0..indexed_columns.len() {
                let size = get_column_size(column_sizes[j]);
                let ctype = get_column_type(column_sizes[j]);
                if ctype == 13 {
                    let content_bytes = buffer[current_offset..current_offset+size].to_vec();
                    current_offset += size;
                    let content = String::from_utf8(content_bytes)?;
                    index_cols.push(Literal::StringLiteral(content));
                } else if ctype == 8 {
                    index_cols.push(Literal::NumberLiteral(0.0));
                } else if ctype == 9 {
                    index_cols.push(Literal::NumberLiteral(1.0));
                } else if ctype == 0 {
                    index_cols.push(Literal::Null);
                } else if ctype > 0 && ctype < 8 {
                    let n = get_num_size_n(&buffer[current_offset..], size, ctype == 7);
                    current_offset += size;
                    index_cols.push(Literal::NumberLiteral(n));
                } else {
                    bail!("Ctype not recognized")
                }
            }
            let searching_col = &index_cols[where_col_num];
            let filtered_value = &where_expr.get_equality()?.1;
            if filtered_value < searching_col {
                break;
            } else if filtered_value > searching_col {
            } else {
                let rowid = get_u64_from_size_n(&buffer[current_offset..], rowid_size as usize);
                read_page(file, table_page_num, page_size, &Some(where_expr.clone()), available_columns, print_columns, Some(rowid))?;
            }
        }
    }
    // internal index
    else if page_type == 2 {
        let last_page = u32::from_be_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);
        let mut search_right = true;
        for i in 0..cell_num as usize {
            let mut current_offset = u16::from_be_bytes([buffer[12+2*i], buffer[12+2*i+1]]) as usize;
            let cell_page = u32::from_be_bytes([buffer[current_offset], buffer[current_offset+1], buffer[current_offset+2], buffer[current_offset+3]]);
            current_offset += 4;
            get_varint(&buffer, &mut current_offset); // payload size
            // start payload
            let header_start = current_offset;
            let header_size = get_varint(&buffer, &mut current_offset) as usize; // header size
            // start header
            let mut column_sizes = vec![];
            for _ in 0..indexed_columns.len() {
                let csize = get_varint(&buffer, &mut current_offset);
                column_sizes.push(csize);
            }
            let rowid_size = get_varint(&buffer, &mut current_offset);
            column_sizes.push(rowid_size);
            if header_start + header_size != current_offset {
                bail!("bad header, currently in {}, expected {}", current_offset, header_start + header_size)
            }
            let mut index_cols = vec![];
            for j in 0..indexed_columns.len() {
                let size = get_column_size(column_sizes[j]);
                let ctype = get_column_type(column_sizes[j]);
                if ctype == 13 {
                    let content_bytes = buffer[current_offset..current_offset+size].to_vec();
                    current_offset += size;
                    let content = String::from_utf8(content_bytes)?;
                    index_cols.push(Literal::StringLiteral(content));
                } else if ctype == 8 {
                    index_cols.push(Literal::NumberLiteral(0.0));
                } else if ctype == 9 {
                    index_cols.push(Literal::NumberLiteral(1.0));
                } else if ctype == 0 {
                    index_cols.push(Literal::Null);
                } else if ctype > 0 && ctype < 8 {
                    let n = get_num_size_n(&buffer[current_offset..], size, ctype == 7);
                    current_offset += size;
                    index_cols.push(Literal::NumberLiteral(n));
                } else {
                    bail!("Ctype not recognized")
                }
            }
            let searching_col = &index_cols[where_col_num];
            let filtered_value = &where_expr.get_equality()?.1;
            if filtered_value < searching_col {
                read_index(file, cell_page, page_size, where_expr, available_columns, print_columns, indexed_columns, table_page_num)?;
                search_right = false;
                if !(&Literal::Null == searching_col) {
                    break;
                }
            } else if filtered_value > searching_col {
            } else {
                let rowid = get_u64_from_size_n(&buffer[current_offset..], rowid_size as usize);
                read_page(file, table_page_num, page_size, &Some(where_expr.clone()), available_columns, print_columns, Some(rowid))?;
                read_index(file, cell_page, page_size, where_expr, available_columns, print_columns, indexed_columns, table_page_num)?;
            }
        }
        if search_right {
            read_index(file, last_page, page_size, where_expr, available_columns, print_columns, indexed_columns, table_page_num)?;
        }
    } else {
        bail!("({}) Unrecognized index page type: {}", line!(), page_type);
    }
    Ok(())
}

pub fn read_page(file: &File, page_num: u32, page_size: usize, where_expr: &Option<Expr>, available_columns: &Vec<Column>, print_columns: &Vec<usize>, search_rowid: Option<u64>) -> Result<()> {
    let mut buffer = Vec::new();
    buffer.resize(page_size, 0u8);
    let page_offset = (page_size*(page_num as usize - 1)) as u64;
    file.read_exact_at(&mut buffer, page_offset)?; // read page
    
    let page_type = buffer[0];
    let cell_num = u16::from_be_bytes([buffer[3], buffer[4]]);

    // leaf page
    if page_type == 13 {
        // read each cell
        for i in 0..cell_num as usize {
            let mut current_offset = u16::from_be_bytes([buffer[8+2*i], buffer[8+2*i+1]]) as usize;
            get_varint(&buffer, &mut current_offset); // size of record
            let rowid = get_varint(&buffer, &mut current_offset) as u64; // the rowid
            if let Some(search_rowid) = search_rowid {
                if rowid < search_rowid {
                    continue;
                } else if rowid > search_rowid {
                    break;
                }
            }
            
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
            
            let mut cols = vec![];
            
            // read record content (loop for each column)
            for j in 0..available_columns.len() {
                if available_columns[j].name == "id" {
                    cols.push(Literal::NumberLiteral(rowid as f64));
                    continue;
                }
                let size = get_column_size(column_sizes[j]);
                let ctype = get_column_type(column_sizes[j]);
                if ctype == 13 {
                    let content_bytes = buffer[current_offset..current_offset+size].to_vec();
                    current_offset += size;
                    let content = String::from_utf8(content_bytes)?;
                    cols.push(Literal::StringLiteral(content));
                } else if ctype == 8 {
                    cols.push(Literal::NumberLiteral(0.0));
                } else if ctype == 9 {
                    cols.push(Literal::NumberLiteral(1.0));
                } else if ctype == 0 {
                    cols.push(Literal::Null);
                } else if ctype > 0 && ctype < 8 {
                    let n = get_num_size_n(&buffer[current_offset..], size, ctype == 7);
                    current_offset += size;
                    cols.push(Literal::NumberLiteral(n));
                } else {
                    bail!("Ctype not recognized")
                }
            }
    
            // check where clause
            if let Some(expr) = where_expr.clone() {
                if let Expr::Equality { column: Literal::Ident(column), condition } = expr {
                    let cnum = available_columns.iter().position(|c| c.name == column).ok_or(anyhow!("Column not on table"))?;
                    let cvalue = &cols[cnum];
                    if cvalue != &condition {
                        continue;
                    }
                }
            }
            
            // print row
            for (j, col_index) in print_columns.iter().enumerate() {
                if j > 0 {
                    print!("|");
                }
                print!("{}", cols[*col_index]);
            }
    
            println!();
        }
    }
    // interior page
    else if page_type == 5 {
        let last_page = u32::from_be_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);
        let mut search_right = true;
        for i in 0..cell_num as usize {
            let mut current_offset = u16::from_be_bytes([buffer[12+2*i], buffer[12+2*i+1]]) as usize;
            let cell_page = u32::from_be_bytes([buffer[current_offset], buffer[current_offset+1], buffer[current_offset+2], buffer[current_offset+3]]);
            current_offset += 4;
            let rowid = get_varint(&buffer, &mut current_offset) as u64; // the rowid
            if let Some(search_rowid) = search_rowid {
                if rowid < search_rowid {
                    continue;
                } else if rowid > search_rowid {
                    read_page(file, cell_page, page_size, where_expr, available_columns, print_columns, Some(search_rowid))?;
                    search_right = false;
                    break;
                }
            }
            read_page(file, cell_page, page_size, where_expr, available_columns, print_columns, search_rowid)?;
        }
        if search_right {
            read_page(file, last_page, page_size, where_expr, available_columns, print_columns, search_rowid)?;
        }
    } else {
        bail!("Unrecognized page type")
    }
    Ok(())
}