#[derive(Debug)]
pub struct StringPair<'a> {
    name: &'a str,
    value: &'a str,
}

impl<'a> StringPair<'a> {
    pub fn new(name: &'a str, value: &'a str) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &'a str {
        self.name
    }

    pub fn value(&self) -> &'a str {
        self.value
    }
}
