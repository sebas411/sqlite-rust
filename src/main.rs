use anyhow::{bail, Result};
use std::fs::File;
use std::io::prelude::*;

fn get_varint(data: &[u8], current_offset: &mut usize) -> i64 {
    let mut out: i64 = 0;
    let mut byte_num = 0;
    for byte in &data[*current_offset..] {
        byte_num += 1;
        if *byte > 127 && byte_num < 9 {
            let tmp = byte & (255-128);
            out <<= 7;
            out |= tmp as i64;
        } else {
            out <<= 8;
            out |= *byte as i64;
            break;
        }
    }
    *current_offset += byte_num;
    out
}

fn main() -> Result<()> {
    // Parse arguments
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        0 | 1 => bail!("Missing <database path> and <command>"),
        2 => bail!("Missing <command>"),
        _ => {}
    }

    // Parse command and act accordingly
    let command = &args[2];
    match command.as_str() {
        ".dbinfo" => {
            let mut file = File::open(&args[1])?;
            let mut header = [0; 100];
            file.read_exact(&mut header)?;

            // The page size is stored at the 16th byte offset, using 2 bytes in big-endian order
            let page_size = u16::from_be_bytes([header[16], header[17]]);

            let mut buffer = Vec::new();
            buffer.resize(page_size as usize, 0u8);

            file.read_exact(&mut buffer[100..])?;

            let table_num = u16::from_be_bytes([buffer[100+3], buffer[100+4]]);

            println!("database page size: {}", page_size);
            println!("number of tables: {}", table_num);
        }
        ".tables" => {
            let mut file = File::open(&args[1])?;
            let mut header = [0; 100];
            file.read_exact(&mut header)?;

            // The page size is stored at the 16th byte offset, using 2 bytes in big-endian order
            let page_size = u16::from_be_bytes([header[16], header[17]]);

            let mut buffer = Vec::new();
            buffer.resize(page_size as usize, 0u8);

            file.read_exact(&mut buffer[100..])?;

            let mut table_names = vec![];

            let table_num = u16::from_be_bytes([buffer[100+3], buffer[100+4]]);
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
                get_varint(&buffer, &mut current_offset); // sqlite_schema.sql
                current_offset += record_offset as usize;
                let tbl_name = buffer[current_offset..current_offset+tbl_name_size].to_vec();
                let tbl_name_string = String::from_utf8(tbl_name)?;
                table_names.push(tbl_name_string);
            }
            
            for table_name in table_names {
                print!("{} ", table_name)
            }
            println!()
        }
        _ => bail!("Missing or invalid command passed: {}", command),
    }

    Ok(())
}
