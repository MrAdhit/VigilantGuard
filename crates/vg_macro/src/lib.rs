use std::collections::HashMap;

use chrono::{Utc, Datelike, Timelike};
use proc_macro::{TokenStream};
use quote::__private::Span;
use syn::{parse_macro_input, DeriveInput, Ident};
use rand::{distributions::Alphanumeric, Rng};

extern crate proc_macro;

extern crate syn;
#[macro_use]
extern crate quote;

#[proc_macro_derive(PacketToBuffer)]
pub fn derive_to_buffer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_len = Ident::new(&format!("LenPacket{}", input.ident.to_string()), Span::call_site());
    let lifetimes = input.generics.lifetimes().next();

    if let Some(lifetime) = lifetimes {
        let expanded = quote! {
            #[derive(Debug, Encode, Decode)]
            struct #name_len {
                len: VarInt
            }

            impl<#lifetime> ToBuffer for #name<#lifetime> {
                fn to_buffer(&mut self) -> Vec<u8> {
                    let mut writer = Vec::new();
                    self.encode(&mut writer).unwrap();
                    let mut writer = &writer[..];
                    #name_len::decode(&mut writer).unwrap();
                    self.len = VarInt(writer.len() as i32);
                    let mut writer = Vec::new();
                    self.encode(&mut writer).unwrap();
                    writer
                }
            }
        };

        TokenStream::from(expanded)
    } else {
        let expanded = quote! {    
            #[derive(Debug, Encode, Decode)]
            struct #name_len {
                len: VarInt
            }

            impl #name {
                pub fn to_buffer(&mut self) -> Vec<u8> {
                    let mut writer = Vec::new();
                    self.encode(&mut writer).unwrap();
                    let mut writer = &writer[..];
                    #name_len::decode(&mut writer).unwrap();
                    self.len = VarInt(writer.len() as i32);
                    let mut writer = Vec::new();
                    self.encode(&mut writer).unwrap();
                    writer
                }
            }
        };

        TokenStream::from(expanded)
    }
}

#[proc_macro_attribute]
pub fn parse_packet_header(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let ret = item.to_string().replace("{", "{pub len : VarInt , pub packet_id : VarInt , ");
    ret.parse().unwrap()
}

enum HashMapParse {
    KEY,
    VAL
}

#[proc_macro]
pub fn random_id(input: TokenStream) -> TokenStream {
    let res: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(5)
        .map(char::from)
        .collect();

    let time = Utc::now();

    let year = time.year();
    let month = time.month();
    let date = time.day();

    let hour = time.hour();
    let minute = time.minute();

    format!("const {}: &str = \"{year}-{month:0>2}-{date:0>2}_{hour:0>2}-{minute:0>2}_{}\";", input.to_string().replace("\"", ""), res.to_lowercase()).parse().unwrap()
}

#[proc_macro]
pub fn create_colorizer(input: TokenStream) -> TokenStream {
    let mut map: HashMap<String, String> = HashMap::new();

    let mut stage = HashMapParse::KEY;
    let mut temp_persist = String::new();
    for (_, t) in input.into_iter().enumerate() {
        if !t.to_string().contains("\"") { continue; }
        let ident = t.to_string().replace("\"", "");

        match &stage {
            HashMapParse::KEY => {
                temp_persist = ident;
                stage = HashMapParse::VAL;
            }
            HashMapParse::VAL => {
                map.insert(temp_persist.clone(), ident);
                stage = HashMapParse::KEY;
            },
        }
    }

    let mut rpl_vec_n = Vec::new();
    let mut rpl_vec = Vec::new();

    for (k, v) in map.clone() {
        rpl_vec.push(format!(".replace(\"c({k})\", \"{v}\")"));
    }

    for (k, v) in map {
        rpl_vec_n.push(format!(".replace(\"c({k})\", \"\")"));
    }
    
    let rpl_n = rpl_vec_n.join(" ");
    let rpl = rpl_vec.join("");
    let res = format!(r###"#[macro_export]{}macro_rules! colorizer {{($fmt_str:literal) => {{{{if crate::file::VIGILANT_CONFIG.colorize {{format!($fmt_str){rpl}}} else {{ format!($fmt_str){rpl_n} }} }}}};($fmt_str:literal, $($args:expr),*) => {{{{if crate::file::VIGILANT_CONFIG.colorize {{ format!($fmt_str, $($args),*){rpl} }} else {{ format!($fmt_str, $($args),*){rpl_n} }} }}}};}}{}pub use colorizer as coloriser;"###, "\n", "\n");

    res.parse().unwrap()
}