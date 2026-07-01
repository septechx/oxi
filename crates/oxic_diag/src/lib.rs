#![feature(proc_macro_tracked_path)]

use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use proc_macro::{Span, TokenStream, tracked::path as track_path};
use proc_macro2::{Span as Span2, TokenStream as TokenStream2};
use quote::quote;
use serde::Deserialize;
use syn::{Ident, LitStr, parse_macro_input};

#[derive(Deserialize)]
struct Diagnostic {
    level: String,
    message: String,
}

type Diagnostics = HashMap<String, Diagnostic>;

#[proc_macro]
pub fn include_diagnostics(input: TokenStream) -> TokenStream {
    let resource_str = parse_macro_input!(input as LitStr);
    let resource_span = resource_str.span().unwrap();
    let relative_path = resource_str.value();
    let Some(absolute_path) = relative_to_absolute(resource_span, &relative_path) else {
        // rust-analyzer doesn't support getting the source file path, so we just return an empty
        // module. https://github.com/rust-lang/rust-analyzer/issues/15950
        return quote! { pub mod diag {} }.into();
    };

    track_path(&absolute_path);

    let resource = read_to_string(absolute_path).expect("Could not read diagnostics file");

    let diagnostics: Diagnostics =
        toml::from_str(&resource).expect("Could not parse diagnostics file");

    let mut body = TokenStream2::new();
    for (key, diagnostic) in diagnostics {
        if !validate_key(&key) {
            panic!("Invalid key: {}", key);
        }
        if !validate_level(&diagnostic.level) {
            panic!("Invalid level: {}", diagnostic.level);
        }

        let pascal_key = Ident::new(&snake_to_pascal(&key), Span2::call_site());
        let level = Ident::new(&diagnostic.level, Span2::call_site());
        let message = diagnostic.message;

        let key_lit = LitStr::new(&key, Span2::call_site());
        let message_lit = LitStr::new(&message, Span2::call_site());

        body.extend(quote! {
            pub struct #pascal_key;
            impl crate::errors::DiagEntry for #pascal_key {
                fn code(&self) -> &'static str { #key_lit }
                fn level(&self) -> crate::errors::ErrorLevel { crate::errors::ErrorLevel::#level }
                fn message(&self) -> &'static str { #message_lit }
            }
        });
    }

    quote! {
        pub mod diag {
            #body
        }
    }
    .into()
}

fn validate_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn validate_level(level: &str) -> bool {
    matches!(level, "Warning" | "Error" | "Fatal")
}

fn relative_to_absolute(span: Span, relative_path: &str) -> Option<PathBuf> {
    let path = Path::new(relative_path);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        let mut source_file_path = span.local_file()?;
        source_file_path.pop();
        source_file_path.push(relative_path);
        source_file_path
    })
}

fn snake_to_pascal(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect()
}
