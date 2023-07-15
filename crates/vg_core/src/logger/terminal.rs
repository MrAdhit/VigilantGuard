use std::collections::VecDeque;
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
use crate::{CONNECTIONS, PLAYERS, RUNTIME, TOTAL_DOWNLOAD, TOTAL_UPLOAD};

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

                        let mut line = line.split(" ").collect::<VecDeque<&str>>();
                        let cmd = line.pop_front().unwrap_or(&"");
                        let args = line;

                        match cmd {
                            "stop" | "exit" => {
                                info!("{}", coloriser!("c(bright_red)Stopping"));
                                std::process::exit(0);
                            }
                            "list" => {
                                let list_type = args.get(0).unwrap_or(&&"");

                                match *list_type {
                                    "connection" | "conn" => {
                                        RUNTIME.spawn(async move {
                                            let lock = CONNECTIONS.lock().await;
                                            info!("{} Connections: {:?}", lock.len(), lock);
                                        });
                                    }
                                    "player" => {
                                        RUNTIME.spawn(async move {
                                            let lock = PLAYERS.lock().await;
                                            let list = lock.values().collect::<Vec<&String>>();
                                            info!("{} Players: {:?}", list.len(), list);
                                        });
                                    }
                                    _ => {
                                        if list_type.len() > 0 {
                                            info!("Unknown subcommand {:?}", list_type);
                                        } else {
                                            info!("Usage: list [connection, player]");
                                        }
                                    }
                                }
                            }
                            "usage" => {
                                let usage_type = args.get(0).unwrap_or(&&"");

                                match *usage_type {
                                    "network" | "net" => unsafe {
                                        info!("\x1b[1;32;42m ⬇ {}MB \x1b[0m\x1b[1;33;43m ⬆ {}MB ", TOTAL_DOWNLOAD.load(Ordering::Relaxed) / 1e+6, TOTAL_UPLOAD.load(Ordering::Relaxed) / 1e+6);
                                    },
                                    _ => {
                                        if usage_type.len() > 0 {
                                            info!("Unknown subcommand {:?}", usage_type);
                                        } else {
                                            info!("Usage: usage [network]");
                                        }
                                    }
                                }
                            }
                            _ => {
                                if cmd.len() > 0 {
                                    info!("Unknown command {:?}", cmd);
                                }
                            }
                        }
                    }
                    Err(err) => {
                        if let ReadlineError::Interrupted = err {
                            std::process::exit(0);
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
