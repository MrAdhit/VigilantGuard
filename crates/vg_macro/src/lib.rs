use proc_macro::{TokenStream};
use quote::__private::Span;
use syn::{parse_macro_input, DeriveInput, Lifetime, Ident};

extern crate proc_macro;

extern crate syn;
#[macro_use]
extern crate quote;

#[proc_macro_derive(PacketToBuffer)]
pub fn derive_to_buffer(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let lifetimes = input.generics.lifetimes().next();

    if let Some(lifetime) = lifetimes {
        let expanded = quote! {
            impl<#lifetime> ToBuffer for #name<#lifetime> {
                fn to_buffer(&mut self) -> Vec<u8> {
                    let mut writer = Vec::new();
                    self.encode(&mut writer).unwrap();
                    let mut writer = &writer[..];
                    LenPacket::decode(&mut writer).unwrap();
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
            impl #name {
                pub fn to_buffer(&mut self) -> Vec<u8> {
                    let mut writer = Vec::new();
                    self.encode(&mut writer).unwrap();
                    let mut writer = &writer[..];
                    LenPacket::decode(&mut writer).unwrap();
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