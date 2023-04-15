use std::{fs::File, io::{ErrorKind, Read, Write, Seek}};

use log::info;

pub static mut IP_BLACKLIST_DB: once_cell::unsync::Lazy<IPBlacklist> = once_cell::unsync::Lazy::new(|| { IPBlacklist::load() });
pub static mut IP_WHITELIST_DB: once_cell::unsync::Lazy<IPWhitelist> = once_cell::unsync::Lazy::new(|| { IPWhitelist::load() });

pub struct IPBlacklist {
    file: File,
    items: Vec<String>
}

impl IPBlacklist {
    fn load() -> Self {
        let mut file = File::options().read(true).write(true).create(true).open("ip_blacklist.db.txt").unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        let items: Vec<String> = buf.split("|").into_iter().map(|v| v.to_string()).collect();
        Self {
            file,
            items
        }
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

pub struct IPWhitelist {
    file: File,
    items: Vec<String>
}

impl IPWhitelist {
    fn load() -> Self {
        let mut file = File::options().read(true).write(true).create(true).open("ip_whitelist.db.txt").unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        let items: Vec<String> = buf.split("|").into_iter().map(|v| v.to_string()).collect();
        Self {
            file,
            items
        }
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