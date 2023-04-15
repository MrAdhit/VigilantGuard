use log::info;

use crate::db::{IP_BLACKLIST_DB, IP_WHITELIST_DB};

pub fn ip_blacklisted(ip: String) -> bool {
    if unsafe { IP_BLACKLIST_DB.has(&ip) } { return true }
    if unsafe { IP_WHITELIST_DB.has(&ip) } { return false }

    let resp = reqwest::blocking::get(format!("https://proxycheck.io/v2/{ip}?vpn=2&asn=0&risk=1")).unwrap().text().unwrap();
    let risk: u8 = resp.split("\"risk\": ").collect::<Vec<&str>>()[1].split("\n").into_iter().collect::<Vec<&str>>()[0].parse().unwrap();

    if risk >= 50 {
        unsafe { IP_BLACKLIST_DB.push(ip) }
        return true;
    } else {
        unsafe { IP_WHITELIST_DB.push(ip) }
        return false;
    }

    false
}