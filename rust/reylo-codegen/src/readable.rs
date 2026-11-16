//! The items as a file shows them to a reader: the prelude's names written short (`Option`,
//! `String`, `Vec`, `Result`, the derives), where the expansion writes each in full
//! (`::core::option::Option`) because it sits beside the crate's own items, which may shadow one. A
//! generated file holds no item of the crate's, so only its own could: where it declares one of
//! those names, every path stays in full.

use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};
use syn::{Item, Path, Token};

/// The names the prelude gives, by the module the expansion writes them from.
const PRELUDE: &[(&[&str], &[&str])] = &[
    (&["core", "option"], &["Option", "Some", "None"]),
    (&["core", "result"], &["Result", "Ok", "Err"]),
    (&["std", "vec"], &["Vec"]),
    (&["std", "string"], &["String"]),
    (&["core", "clone"], &["Clone"]),
    (&["core", "fmt"], &["Debug"]),
    (&["core", "cmp"], &["PartialEq"]),
    (&["core"], &["assert", "include_str"]),
];

/// Writes `file`'s paths short, unless it declares a name of the prelude's itself.
pub fn readable(file: &mut syn::File) {
    let mut declared = Vec::new();
    collect(&file.items, &mut declared);
    let shadowed = PRELUDE.iter().flat_map(|(_, names)| names.iter()).any(|name| declared.iter().any(|d| d == name));
    Short { prelude: !shadowed }.visit_file_mut(file);
}

fn collect(items: &[Item], out: &mut Vec<String>) {
    for item in items {
        match item {
            Item::Struct(s) => out.push(s.ident.to_string()),
            Item::Enum(e) => out.push(e.ident.to_string()),
            Item::Mod(m) => collect(m.content.as_ref().map_or(&[][..], |(_, items)| items), out),
            _ => {}
        }
    }
}

struct Short {
    prelude: bool,
}

impl Short {
    fn shorten(&self, path: &mut Path) {
        if path.leading_colon.is_none() {
            return;
        }
        let names: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        // The crate's own dependency, `reylo`, which no generated item shadows.
        if names.first().is_some_and(|n| n == "reylo") {
            path.leading_colon = None;
            return;
        }
        if !self.prelude {
            return;
        }
        for (module, items) in PRELUDE {
            let n = module.len();
            if names.len() > n && names[..n].iter().zip(module.iter()).all(|(a, b)| a == b) && items.contains(&names[n].as_str()) {
                path.leading_colon = None;
                path.segments = path.segments.iter().skip(n).cloned().collect();
                return;
            }
        }
    }
}

impl VisitMut for Short {
    fn visit_path_mut(&mut self, path: &mut Path) {
        visit_mut::visit_path_mut(self, path);
        self.shorten(path);
    }

    // A derive's paths are the attribute's tokens, which the visitor does not read as paths.
    fn visit_attribute_mut(&mut self, attr: &mut syn::Attribute) {
        if attr.path().is_ident("derive") {
            if let Ok(mut paths) = attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated) {
                for path in paths.iter_mut() {
                    self.shorten(path);
                }
                attr.meta = syn::parse_quote!(derive(#paths));
            }
        }
        visit_mut::visit_attribute_mut(self, attr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn shown(source: &str) -> String {
        let mut file: syn::File = syn::parse_str(source).unwrap();
        readable(&mut file);
        file.to_token_stream().to_string()
    }

    #[test]
    fn the_preludes_names_are_written_short() {
        assert_eq!(
            shown("#[derive(::core::fmt::Debug, ::core::clone::Clone)] pub struct Row { pub a: ::core::option::Option<::std::string::String>, pub b: ::reylo::sqlx::types::Uuid }"),
            "# [derive (Debug , Clone)] pub struct Row { pub a : Option < String > , pub b : reylo :: sqlx :: types :: Uuid }"
        );
        assert_eq!(shown("fn f() { ::core::result::Result::Ok(::core::clone::Clone::clone(&x)) }"), "fn f () { Result :: Ok (Clone :: clone (& x)) }");
    }

    #[test]
    fn a_file_that_declares_a_preludes_name_keeps_every_path_in_full() {
        assert_eq!(shown("pub enum Result { A } pub struct Row { pub r: ::core::option::Option<Result> }"), "pub enum Result { A } pub struct Row { pub r : :: core :: option :: Option < Result > }");
    }
}
