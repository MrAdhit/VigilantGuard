mod ip_filter_file;

use once_cell::sync::Lazy;

use self::ip_filter_file::IpFilter;

pub static mut IP_BLACKLIST_DB: Lazy<IpFilter> = Lazy::new(|| { IpFilter::load("ip_blacklist.db.txt") });
pub static mut IP_WHITELIST_DB: Lazy<IpFilter> = Lazy::new(|| { IpFilter::load("ip_whitelist.db.txt") });
