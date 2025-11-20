
pub struct Table {
    pub name: String,
    pub rootpage: u8,
    pub columns: Vec<Column>
}

impl Table {
    pub fn new(name: &str, rootpage: u8, columns: Vec<Column>) -> Self {
        Self { name: name.into(), rootpage, columns }
    }
}

pub struct Column {
    pub name: String,
    pub _ctype: String,
}

impl Column {
    pub fn new(name: &str, ctype: &str) -> Self {
        Self { name: name.into(), _ctype: ctype.into() }
    }
}
