use std::fs::{
    File, {self},
};
use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub colorize: bool,
    pub proxy: ProxyConfig,
    pub server: ServerConfig,
    pub guardian: GuardianConfig,
}

#[derive(Serialize, Deserialize)]
pub struct ProxyConfig {
    pub ip: String,
    pub port: u16,
    pub forwarder: ProxyForwarder,
}

#[derive(Serialize, Deserialize)]
pub struct ProxyForwarder {
    pub ip_forward: bool,
    pub ping_forward: bool,
    pub motd_forward: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ServerConfig {
    pub ip: String,
    pub port: u16,
}

#[derive(Serialize, Deserialize)]
pub struct GuardianConfig {
    pub ping_protection: PingProtection,
    pub ip_connection_limit: IPLimiter,
    pub vpn_filter: VPNFilter,
}

#[derive(Serialize, Deserialize)]
pub struct PingProtection {
    pub active: bool,
    pub reset_interval: u64,
}

#[derive(Serialize, Deserialize)]
pub struct IPLimiter {
    pub active: bool,
    pub limit: usize,
}

#[derive(Serialize, Deserialize)]
pub struct VPNFilter {
    pub active: bool,
}

impl Config {
    pub fn save(&self) {
        if let Ok(_) = fs::read("./config.toml") {
            fs::rename("./config.toml", "./config.toml.bak").unwrap();
        }

        let mut file = File::options().read(false).write(true).create(true).open("./config.toml").unwrap();
        let config = toml::to_string_pretty(&self).unwrap();
        file.write_all(config.as_bytes()).unwrap();
    }
}

pub fn parse() -> Config {
    let mut file = File::options().read(true).write(true).create(true).open("./config.toml").unwrap();
    let mut buf = String::new();

    file.read_to_string(&mut buf).unwrap();

    let config: Result<Config, toml::de::Error> = toml::from_str(&buf);

    if let Ok(config) = config {
        return config;
    } else {
        let default: Config = toml::from_str(&String::from_utf8_lossy(DEFAULT_CONFIG.as_bytes())).unwrap();
        default.save();
        return default;
    }
}

const DEFAULT_CONFIG: &str = r##"
colorize = true

[proxy]
ip = "0.0.0.0"
port = 25565

[proxy.forwarder]
ip_forward = true
ping_forward = false
motd_forward = false

[server]
ip = "127.0.0.1"
port = 25567

[guardian.ping_protection]
active = false
reset_interval = 300 # In Seconds

[guardian.ip_connection_limit]
active = false
limit = 3

[guardian.vpn_filter]
active = false

"##;
