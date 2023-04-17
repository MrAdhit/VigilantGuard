use log::info;

use crate::file::*;

pub async fn ip_blacklisted(ip: String) -> bool {
    if ip == "127.0.0.1" { return false }
    if unsafe { IP_BLACKLIST_DB.has(&ip) } { return true }
    if unsafe { IP_WHITELIST_DB.has(&ip) } { return false }

    let resp = reqwest::get(format!("https://proxycheck.io/v2/{ip}?vpn=2&asn=0&risk=1")).await.unwrap().text().await.unwrap();
    let risk: u8 = resp.split("\"risk\": ").collect::<Vec<&str>>()[1].split("\n").into_iter().collect::<Vec<&str>>()[0].parse().unwrap_or(0);

    if risk >= 50 {
        unsafe { IP_BLACKLIST_DB.push(ip) }
        return true;
    } else {
        unsafe { IP_WHITELIST_DB.push(ip) }
        return false;
    }
}