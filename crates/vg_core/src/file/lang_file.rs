use std::{fs::{File, self}, io::{Read, Write}};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Lang {
    pub player_ping_not_cached_kick: String,
    pub player_connection_more_kick: String,
    pub player_ip_blacklisted_kick: String,
}

impl Lang {
    pub fn save(&self) {
        if let Ok(_) = fs::read("./lang.toml") {
            fs::rename("./lang.toml", "./lang.toml.bak").unwrap();
        }

        let mut file = File::options().read(false).write(true).create(true).open("./lang.toml").unwrap();
        let config = toml::to_string_pretty(&self).unwrap();
        file.write_all(config.as_bytes()).unwrap();
    }

    pub fn parse_color(&self) -> Self {
        Self {
            player_ping_not_cached_kick: self.player_ping_not_cached_kick.replace("&", "§"),
            player_connection_more_kick: self.player_connection_more_kick.replace("&", "§"),
            player_ip_blacklisted_kick: self.player_ip_blacklisted_kick.replace("&", "§"),
        }
    }
}

pub fn parse() -> Lang {
    log::info!("Loading language file");
    let mut file = File::options().read(true).write(true).create(true).open("./lang.toml").unwrap();
    let mut buf = String::new();

    file.read_to_string(&mut buf).unwrap();

    let config: Result<Lang, toml::de::Error> = toml::from_str(&buf);

    if let Ok(config) = config {
        return config;
    } else {
        let default: Lang = toml::from_str(&String::from_utf8_lossy(DEFAULT_LANG.as_bytes())).unwrap();
        default.save();
        return default;
    }
}

const DEFAULT_LANG: &str = r##"
player_ping_not_cached_kick = "&c&lPlease Refresh and Rejoin!"
player_connection_more_kick = "&c&lYou have excedeed the max connection allowed!"
player_ip_blacklisted_kick = "&c&lYou may have used a VPN\\n&c&lplease contact admin to resolve this issue"

"##;