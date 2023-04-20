use std::fs::File;
use std::io::{Read, Seek, Write};

pub struct IpFilter {
    file: File,
    items: Vec<String>,
}

impl IpFilter {
    pub fn load(out_file: &str) -> Self {
        let mut file = File::options().read(true).write(true).create(true).open(out_file).unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        let items: Vec<String> = buf.split("|").into_iter().map(|v| v.to_string()).collect();
        Self { file, items }
    }

    pub fn push<S: Into<String>>(&mut self, item: S) {
        let item: String = item.into();
        if !self.has(&item) {
            self.items.push(item);
            self.update();
        }
    }

    pub fn remove<S: Into<String>>(&mut self, item: S) {
        let item: String = item.into();
        if let Some(index) = self.items.iter().enumerate().find_map(|(i, v)| if v == &item { Some(i) } else { None }) {
            self.items.remove(index);
        }
    }

    pub fn has<S: Into<String>>(&self, item: S) -> bool {
        let item: String = item.into();
        if let None = self.items.iter().find(|&v| v == &item) {
            return false;
        } else {
            return true;
        }
    }

    pub fn update(&mut self) {
        let val = self.items.join("|");
        let buf = val.as_bytes();
        self.file.set_len(0).unwrap();
        self.file.rewind().unwrap();
        self.file.write(buf).unwrap();
    }
}
