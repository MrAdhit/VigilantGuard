mod config_file;
mod ip_filter_file;
pub mod lang_file;

use once_cell::sync::Lazy;

use self::config_file::Config;
use self::ip_filter_file::IpFilter;
use self::lang_file::Lang;

pub static VIGILANT_CONFIG: Lazy<Config> = Lazy::new(|| config_file::parse());
pub static VIGILANT_LANG: Lazy<Lang> = Lazy::new(|| lang_file::parse());

pub static mut IP_BLACKLIST_DB: Lazy<IpFilter> = Lazy::new(|| IpFilter::load("ip_blacklist.db.txt"));
pub static mut IP_WHITELIST_DB: Lazy<IpFilter> = Lazy::new(|| IpFilter::load("ip_whitelist.db.txt"));
