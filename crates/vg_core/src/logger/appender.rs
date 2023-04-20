use std::{sync::Mutex, io::BufWriter};

use log4rs::{encode::{Encode, writer::simple::SimpleWriter}, append::Append};

use crate::file::VIGILANT_CONFIG;

pub struct LogAppender<T: FnMut(String) + Sync + Send + 'static> {
    pub printer: Mutex<T>,
    pub encoder: Box<dyn Encode>
}

impl<F: FnMut(String) + Sync + Send + 'static> Append for LogAppender<F> {
    fn append(&self, record: &log::Record) -> anyhow::Result<()> {
        let mut writer = SimpleWriter(BufWriter::new(Vec::new()));
        self.encoder.encode(&mut writer, record).unwrap();
        let str = String::from_utf8_lossy(writer.0.buffer());
        let color = match record.level() {
            log::Level::Error => "\x1b[1;31m",
            log::Level::Warn => "\x1b[0;33m",
            log::Level::Info => "\x1b[1;32m",
            _ => ""
        };
        if VIGILANT_CONFIG.colorize {
            (self.printer.lock().unwrap())(str.to_string().replace(record.level().as_str(), format!("{}{}\x1b[0m", color, record.level().as_str()).as_str()));
        } else {
            (self.printer.lock().unwrap())(str.to_string());
        }
        Ok(())
    }

    fn flush(&self) {
        todo!()
    }
}

impl<F: FnMut(String) + Sync + Send + 'static> std::fmt::Debug for LogAppender<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogAppender").field("encoder", &self.encoder).finish()
    }
}