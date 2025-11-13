use anyhow::{bail, Result};
use std::fs::File;
use std::io::prelude::*;

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
        _ => bail!("Missing or invalid command passed: {}", command),
    }

    Ok(())
}
