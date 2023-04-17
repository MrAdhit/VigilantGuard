use std::collections::HashMap;

use proc_macro::{TokenStream, Literal};
use quote::__private::Span;
use syn::{parse_macro_input, DeriveInput, Ident};

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
pub fn create_colorizer(input: TokenStream) -> TokenStream {
    let mut map: HashMap<String, String> = HashMap::new();

    let mut stage = HashMapParse::KEY;
    let mut temp_persist = String::new();
    for (i, t) in input.into_iter().enumerate() {
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

    let mut rpl_vec = Vec::new();

    for (k, v) in map {
        rpl_vec.push(format!(".replace(\"c({k})\", \"{v}\")"));
    }
    
    let rpl = rpl_vec.join("");    
    let res = format!(r###"#[macro_export]{}macro_rules! colorizer {{($fmt_str:literal) => {{{{format!($fmt_str){rpl}}}}};($fmt_str:literal, $($args:expr),*) => {{{{format!($fmt_str, $($args),*){rpl}}}}};}}{}pub use colorizer as coloriser;"###, "\n", "\n");

    res.parse().unwrap()
}