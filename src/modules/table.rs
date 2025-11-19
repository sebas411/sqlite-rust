
pub struct Table {
    pub name: String,
    pub rootpage: u8,
}

impl Table {
    pub fn new(name: &str, rootpage: u8) -> Self {
        Self { name: name.into(), rootpage }
    }
}