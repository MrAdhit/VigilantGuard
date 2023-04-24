use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::thread;

use log::{error, info, LevelFilter};
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use rustyline::error::ReadlineError;
use rustyline::{DefaultEditor, ExternalPrinter};

use super::appender::LogAppender;
use crate::macros::coloriser;
use crate::{TOTAL_DOWNLOAD, TOTAL_UPLOAD};

pub fn setup() -> Result<(), ()> {
    let mut rl = DefaultEditor::new().unwrap();
    let mut printer = rl.create_external_printer().unwrap();

    thread::Builder::new()
        .name("command".to_string())
        .spawn(move || {
            loop {
                let line = rl.readline("> ");
                match line {
                    Ok(line) => {
                        rl.add_history_entry(&line).unwrap();

                        match line.as_str() {
                            "stop" => {
                                info!("{}", coloriser!("c(bright_red)Stopping"));
                                std::process::exit(0);
                            }
                            "usage" => unsafe {
                                info!("\x1b[1;32;42m ⬇ {}MB \x1b[0m\x1b[1;33;43m ⬆ {}MB ", TOTAL_DOWNLOAD.load(Ordering::Relaxed) / 1e+6, TOTAL_UPLOAD.load(Ordering::Relaxed) / 1e+6);
                            },
                            _ => {
                                if line.len() > 0 {
                                    info!("Unknown command {:?}", line);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        if let ReadlineError::Interrupted = err {
                            std::process::exit(1);
                        }

                        error!("{}", coloriser!("c(bright_red){}", err.to_string()));
                    }
                }
            }
        })
        .unwrap();

    let patt = "[{d(%H:%M:%S)}] {([{T}/{h({l})}]):<12}: {m}\x1b[0m\n";

    let stdout = LogAppender { printer: Mutex::new(move |v| printer.print(v).unwrap()), encoder: Box::new(PatternEncoder::new(patt)) };

    let config = Config::builder().appenders([Appender::builder().build("stdout", Box::new(stdout))]).build(Root::builder().appenders(["stdout"]).build(LevelFilter::Info)).unwrap();

    let _handle = log4rs::init_config(config).unwrap();

    Ok(())
}
