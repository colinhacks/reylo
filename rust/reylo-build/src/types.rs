//! A type descriptor's Rust type (`spec/type-descriptor.ts`, `engine/core/src/types.rs`).
//!
//! The kind and the fields it requires pick the Rust type, the database's name decides where a
//! client's convention does (a SQLite column declared BOOLEAN is a `bool`, as `columnKind` in
//! `src/values.ts` reads it), and the widening fields say which values the Rust type must hold.
//! Where the usual Rust type holds most of them and not all (a Postgres `date` may hold infinity,
//! and years chrono cannot write), the column is decoded through `reylo::Exact`, which refuses
//! what the type cannot hold when it is read; no value is ever changed. Where no Rust type holds
//! the values at all, the expansion fails at the statement and says how to cast it.
//!
//! A database enum is a Rust enum, which implements `reylo::Enum`: a Postgres enum type, decoded
//! and bound by the enum's own sqlx impls, and a list of values (a `CHECK` constraint's, a MySQL
//! `ENUM`'s, or one the statement names), read as text and bound as its value. Each enum and each
//! struct the module holds is named once every one is known, so that two of them, or one and
//! `Row`, `Params`, `Query` or an item of the crate's own, never take the same name
//! ([`Types::finish`]).

use crate::names;
use crate::{Dates, Decimals, Dialect, Options};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote};
use reylo_engine_core::types::{Kind, Type};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// How a value at the bottom of a column's or a parameter's type is read and written.
#[derive(Clone)]
pub enum Leaf {
    /// As sqlx decodes and encodes its type.
    Plain,
    /// Decoded through `reylo::Exact`, which refuses what the type cannot hold.
    Exact,
    /// One of a list of values: decoded as text and read as the enum's variant, bound as its value.
    Label,
}

/// An enum the module holds, as a column's or a parameter's value: its name in the module (a
/// stand-in until [`Types::finish`] names it), and its values in the database's order.
#[derive(Clone)]
pub struct EnumRef {
    pub ident: Ident,
    pub labels: Vec<String>,
}

/// A column's or a parameter's Rust type: an `Option` where it may be null, a `Vec` for an array.
#[derive(Clone)]
pub enum Shape {
    Leaf { ty: TokenStream2, leaf: Leaf, enumeration: Option<EnumRef> },
    Option(Box<Shape>),
    Vec(Box<Shape>),
}

impl Shape {
    /// The field's type.
    pub fn ty(&self) -> TokenStream2 {
        match self {
            Shape::Leaf { ty, .. } => ty.clone(),
            Shape::Option(s) => {
                let t = s.ty();
                quote!(::core::option::Option<#t>)
            }
            Shape::Vec(s) => {
                let t = s.ty();
                quote!(::std::vec::Vec<#t>)
            }
        }
    }

    /// The type sqlx decodes the column into.
    pub fn decode_ty(&self) -> TokenStream2 {
        match self {
            Shape::Leaf { ty, leaf: Leaf::Exact, .. } => quote!(::reylo::Exact<#ty>),
            Shape::Leaf { leaf: Leaf::Label, .. } => quote!(::std::string::String),
            Shape::Leaf { ty, .. } => ty.clone(),
            Shape::Option(s) => {
                let t = s.decode_ty();
                quote!(::core::option::Option<#t>)
            }
            Shape::Vec(s) => {
                let t = s.decode_ty();
                quote!(::std::vec::Vec<#t>)
            }
        }
    }

    /// The enum at the bottom of the type, if there is one.
    pub fn enumeration(&self) -> Option<&EnumRef> {
        match self {
            Shape::Leaf { enumeration, .. } => enumeration.as_ref(),
            Shape::Option(s) | Shape::Vec(s) => s.enumeration(),
        }
    }

    /// Whether a value of the decode type is already the field's value.
    pub fn decoded_as_is(&self) -> bool {
        match self {
            Shape::Leaf { leaf, .. } => matches!(leaf, Leaf::Plain),
            Shape::Option(s) | Shape::Vec(s) => s.decoded_as_is(),
        }
    }

    /// `v`, of the decode type, as the field's value, inside a function that returns sqlx's
    /// `Result`: a value that is none of a list's values is an error naming the column.
    pub fn decode(&self, v: TokenStream2, column: &str) -> TokenStream2 {
        if self.decoded_as_is() {
            return v;
        }
        match self {
            Shape::Leaf { leaf: Leaf::Exact, .. } => quote!(#v.0),
            Shape::Leaf { ty, leaf: Leaf::Label, .. } => quote!(::reylo::__private::from_label::<#ty>(&#v, #column)?),
            Shape::Leaf { .. } => v,
            Shape::Option(s) => {
                let inner = s.decode(quote!(v), column);
                quote!(match #v { ::core::option::Option::Some(v) => ::core::option::Option::Some(#inner), ::core::option::Option::None => ::core::option::Option::None })
            }
            Shape::Vec(s) => {
                let inner = s.decode(quote!(v), column);
                quote!({ let mut out = ::std::vec::Vec::new(); for v in #v { out.push(#inner); } out })
            }
        }
    }

    /// The value sqlx binds for `v`, a reference to the field's value: a list's value is bound as
    /// the text it is.
    pub fn encode(&self, v: TokenStream2) -> TokenStream2 {
        self.encode_leaves(v, &|leaf, ty, v| match leaf {
            Leaf::Label => quote!(<#ty as ::reylo::Enum>::label(*#v)),
            _ => quote!(::core::clone::Clone::clone(#v)),
        })
    }

    /// `v`, a reference to a value of this shape, with `f` given each value at the bottom by
    /// reference, as a value to bind.
    pub fn encode_leaves(&self, v: TokenStream2, f: &dyn Fn(&Leaf, &TokenStream2, TokenStream2) -> TokenStream2) -> TokenStream2 {
        match self {
            Shape::Leaf { ty, leaf, .. } => f(leaf, ty, v),
            Shape::Option(s) => {
                let inner = s.encode_leaves(quote!(v), f);
                quote!(match #v { ::core::option::Option::Some(v) => ::core::option::Option::Some(#inner), ::core::option::Option::None => ::core::option::Option::None })
            }
            Shape::Vec(s) => {
                let inner = s.encode_leaves(quote!(v), f);
                quote!((#v).iter().map(|v| #inner).collect::<::std::vec::Vec<_>>())
            }
        }
    }

    /// `v`, an owned value of this shape, with `f` applied to each value at the bottom.
    pub fn map_leaves(&self, v: TokenStream2, f: &dyn Fn(TokenStream2) -> TokenStream2) -> TokenStream2 {
        match self {
            Shape::Leaf { .. } => f(v),
            Shape::Option(s) => {
                let inner = s.map_leaves(quote!(v), f);
                quote!(match #v { ::core::option::Option::Some(v) => ::core::option::Option::Some(#inner), ::core::option::Option::None => ::core::option::Option::None })
            }
            Shape::Vec(s) => {
                let inner = s.map_leaves(quote!(v), f);
                quote!(#v.into_iter().map(|v| #inner).collect::<::std::vec::Vec<_>>())
            }
        }
    }

    /// The type at the bottom of `written`, a type the crate wrote for a value of this shape: the
    /// `X` of `Option<X>` where this is an `Option`, and of `Vec<X>` where it is a `Vec`.
    pub fn leaf_of(&self, written: &syn::Type, name: &str) -> syn::Result<syn::Type> {
        match self {
            Shape::Leaf { .. } => Ok(written.clone()),
            Shape::Option(s) => s.leaf_of(&generic_arg(written, "Option", name, "may be null: write its type as `Option<…>`")?, name),
            Shape::Vec(s) => s.leaf_of(&generic_arg(written, "Vec", name, "is an array: write its type as `Vec<…>`")?, name),
        }
    }
}

/// `X` in `written` as `Outer<X>`, or the error that says how to write it.
fn generic_arg(written: &syn::Type, outer: &str, name: &str, hint: &str) -> syn::Result<syn::Type> {
    if let syn::Type::Path(p) = written {
        if let Some(last) = p.path.segments.last().filter(|s| s.ident == outer) {
            if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                if let [syn::GenericArgument::Type(inner)] = args.args.iter().collect::<Vec<_>>()[..] {
                    return Ok(inner.clone());
                }
            }
        }
    }
    Err(syn::Error::new_spanned(written, format!("{name} {hint}")))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    Column,
    Param,
}

/// An item the module holds, waiting for its name: its stand-in, the name it takes where no other
/// item wants it, and the one it takes where another does.
struct Named {
    stand_in: Ident,
    base: String,
    fallback: String,
}

/// A Postgres enum type a generated file declares once, in its schema's module, for every query
/// of the file that reads or binds it.
pub struct SharedEnum {
    /// The type, as `public.mood`.
    pub qualified: String,
    /// The schema's module and the enum's name in it.
    pub schema: String,
    pub name: String,
    /// The enum and its impls, under its name.
    pub tokens: TokenStream2,
}

/// The items a statement's types need, named.
pub struct Finished {
    pub items: Vec<TokenStream2>,
    pub shared: Vec<SharedEnum>,
    names: HashMap<String, Ident>,
}

impl Finished {
    /// `tokens` with every stand-in for an item's name replaced by its name.
    pub fn named(&self, tokens: TokenStream2) -> TokenStream2 {
        tokens
            .into_iter()
            .map(|tree| match tree {
                TokenTree::Ident(ident) => match self.names.get(&ident.to_string()) {
                    Some(name) => TokenTree::Ident(Ident::new(&name.to_string(), ident.span())),
                    None => TokenTree::Ident(ident),
                },
                TokenTree::Group(group) => {
                    let mut inner = proc_macro2::Group::new(group.delimiter(), self.named(group.stream()));
                    inner.set_span(group.span());
                    TokenTree::Group(inner)
                }
                other => other,
            })
            .collect()
    }
}

/// The Rust types of one statement, and the items they need: an enum per database enum, a struct
/// per JSON object the statement builds.
pub struct Types {
    dialect: Dialect,
    options: Options,
    /// Each domain the statement's types are named by, with its base type's name.
    domains: BTreeMap<String, String>,
    span: Span,
    /// Whether a Postgres enum type is declared once for a whole generated file, rather than here.
    shared: bool,
    items: Vec<TokenStream2>,
    shared_enums: Vec<SharedEnum>,
    /// Each enum already written, by its type (`type:public.mood`) or its values (`list:…`).
    enums: HashMap<String, EnumRef>,
    named: Vec<Named>,
    /// Whether every type written so far has a serde form, so the row can derive `Serialize`.
    pub serialize: bool,
}

impl Types {
    pub fn new(dialect: Dialect, options: Options, domains: BTreeMap<String, String>, span: Span, shared: bool) -> Types {
        Types { dialect, options, domains, span, shared, items: Vec::new(), shared_enums: Vec::new(), enums: HashMap::new(), named: Vec::new(), serialize: true }
    }

    /// The items, each named: a name another item, `Row`, `Params`, `Query` or one of `taken` (the
    /// names the crate's own items in the module have) would also take is the item's fallback
    /// (`PublicMood`, `KindValues`, `AuthorObject`), and two fallbacks that meet are an error.
    pub fn finish(self, taken: &BTreeSet<String>) -> syn::Result<Finished> {
        let mut reserved: BTreeSet<String> = taken.clone();
        reserved.extend(["Row", "Params", "Query"].map(String::from));
        let mut wanted: HashMap<&str, usize> = HashMap::new();
        for n in &self.named {
            *wanted.entry(n.base.as_str()).or_default() += 1;
        }
        let mut names = HashMap::new();
        let mut given: BTreeMap<String, &Named> = BTreeMap::new();
        for n in &self.named {
            let name = if wanted[n.base.as_str()] == 1 && !reserved.contains(&n.base) { &n.base } else { &n.fallback };
            if reserved.contains(name) || given.contains_key(name) {
                return Err(syn::Error::new(self.span, format!("two of the types this statement needs would both be named {name}: give one of its columns or parameters another name with `as`")));
            }
            given.insert(name.clone(), n);
            names.insert(n.stand_in.to_string(), format_ident!("{}", name));
        }
        let finished = Finished { items: Vec::new(), shared: Vec::new(), names };
        let items = self.items.into_iter().map(|t| finished.named(t)).collect();
        Ok(Finished { items, shared: self.shared_enums, ..finished })
    }

    /// A column's Rust type, `Option` where it may be null.
    pub fn column(&mut self, t: &Type, nullable: bool, label: &str) -> syn::Result<Shape> {
        let shape = self.value(t, label, Use::Column)?;
        Ok(if nullable { Shape::Option(Box::new(shape)) } else { shape })
    }

    /// A parameter's Rust type, `Option` where it may be null; text is borrowed, a `&'a str`.
    pub fn param(&mut self, t: &Type, nullable: bool, label: &str) -> syn::Result<(Shape, bool)> {
        let (shape, borrowed) = if self.is_string(t) {
            (Shape::Leaf { ty: quote!(&'a str), leaf: Leaf::Plain, enumeration: None }, true)
        } else {
            (self.value(t, label, Use::Param)?, false)
        };
        Ok((if nullable { Shape::Option(Box::new(shape)) } else { shape }, borrowed))
    }

    /// Whether the type's values are text, so that a parameter of it takes a `&str`.
    fn is_string(&self, t: &Type) -> bool {
        match (&t.kind, self.dialect) {
            (Kind::Text(_), _) => true,
            // A list of one text value stays text; a list of more is an enum, a Postgres enum type too.
            (Kind::Enum(e), _) => e.values.len() < 2 && !self.is_postgres_enum(t) && self.is_text_list(&e.ty),
            (Kind::Union(_), Dialect::Sqlite) => t.label.as_deref().is_some_and(|n| !is_bool(n) && affinity(n) == Affinity::Text),
            _ => false,
        }
    }

    fn value(&mut self, t: &Type, label: &str, at: Use) -> syn::Result<Shape> {
        if let Kind::Array(a) = &t.kind {
            if self.plain_array(t) {
                let element = self.value(&a.element.ty, label, at)?;
                return Ok(Shape::Vec(Box::new(if a.element.nullable { Shape::Option(Box::new(element)) } else { element })));
            }
        }
        if let Kind::Enum(e) = &t.kind {
            if self.is_postgres_enum(t) {
                let reference = self.postgres_enum(t)?;
                return Ok(Shape::Leaf { ty: { let i = &reference.ident; quote!(#i) }, leaf: Leaf::Plain, enumeration: Some(reference) });
            }
            // A list of text values a CHECK, a MySQL ENUM or the statement names (`case … then 'a'
            // else 'b' end`): an enum read from its text. A single value, as a literal column, is text.
            if self.is_text_list(&e.ty) && e.values.len() >= 2 {
                let reference = self.list_enum(&e.values, label);
                return Ok(Shape::Leaf { ty: { let i = &reference.ident; quote!(#i) }, leaf: Leaf::Label, enumeration: Some(reference) });
            }
        }
        let (ty, exact) = match self.dialect {
            Dialect::Postgres => self.postgres(t, label)?,
            Dialect::Sqlite => self.sqlite(t, label)?,
            Dialect::Mysql => self.mysql(t, label)?,
        };
        Ok(Shape::Leaf { ty, leaf: if exact && at == Use::Column { Leaf::Exact } else { Leaf::Plain }, enumeration: None })
    }

    /// Whether a list's values are text, which a Rust enum can stand for.
    fn is_text_list(&self, base: &Type) -> bool {
        match &base.kind {
            Kind::Text(_) => true,
            Kind::Enum(e) => self.is_text_list(&e.ty),
            Kind::Union(_) => self.dialect == Dialect::Sqlite && self.is_string(base),
            _ => false,
        }
    }

    /// An array sqlx decodes as a `Vec` of its elements: one the statement builds, or a Postgres
    /// array type (`_int4`); not a type that is an array only in its values, as pgvector's `vector`.
    fn plain_array(&self, t: &Type) -> bool {
        match t.label.as_deref() {
            None => true,
            Some(name) => self.dialect == Dialect::Postgres && bare(name).starts_with('_'),
        }
    }

    /// A Postgres type's Rust type, and whether it is decoded through `reylo::Exact`.
    fn postgres(&mut self, t: &Type, label: &str) -> syn::Result<(TokenStream2, bool)> {
        let name = t.label.as_deref().map(bare).unwrap_or_default();
        Ok(match &t.kind {
            Kind::Boolean(_) => (quote!(bool), false),
            Kind::Integer(i) => match (i.bits, i.unsigned) {
                (8, false) => (quote!(i8), false),
                (16, false) => (quote!(i16), false),
                (32, false) => (quote!(i32), false),
                (64, false) => (quote!(i64), false),
                (32, true) if name == "oid" => {
                    self.serialize = false;
                    (quote!(::reylo::sqlx::postgres::types::Oid), false)
                }
                _ => return Err(self.unsupported(label, t, "")),
            },
            Kind::Float(f) => (float(f.exponent_bits, f.mantissa_bits), false),
            // Read through `reylo::Exact`: a numeric past a Decimal's range is an error, where sqlx
            // 0.8's own decoder panics.
            Kind::Decimal(_) => (self.decimal(label)?, true),
            // A list of values of another type than text: its base type's values.
            Kind::Enum(e) => self.postgres(&e.ty, label)?,
            Kind::Text(_) => (quote!(::std::string::String), false),
            Kind::Bytes(_) if name.is_empty() || name == "bytea" => (quote!(::std::vec::Vec<u8>), false),
            Kind::Uuid(_) => (self.uuid(label)?, false),
            Kind::Json(j) => (self.json_column(j, label)?, false),
            Kind::Date(_) => (self.chrono_or_time(label, quote!(NaiveDate), quote!(Date))?, true),
            Kind::Time(time) if !time.offset => (self.chrono_or_time(label, quote!(NaiveTime), quote!(Time))?, true),
            Kind::DateTime(_) => (self.chrono_or_time(label, quote!(NaiveDateTime), quote!(PrimitiveDateTime))?, true),
            Kind::Instant(_) => (self.chrono_or_time(label, quote!(DateTime<::reylo::sqlx::types::chrono::Utc>), quote!(OffsetDateTime))?, true),
            // Its months, days and microseconds, as the server sends them.
            Kind::Duration(_) => {
                self.serialize = false;
                (quote!(::reylo::sqlx::postgres::types::PgInterval), false)
            }
            Kind::Opaque if name == "money" => {
                self.serialize = false;
                (quote!(::reylo::sqlx::postgres::types::PgMoney), false)
            }
            Kind::Unknown => return Err(self.any_value(label)),
            _ => return Err(self.unsupported(label, t, "cast it to text in the statement, as `x::text`")),
        })
    }

    /// A SQLite type's Rust type. A column that is not STRICT holds what its declared type's
    /// affinity stores, a union the engine names by the declared type; the Rust type is the one
    /// that declaration means, and a value of another storage class is refused when read.
    fn sqlite(&mut self, t: &Type, label: &str) -> syn::Result<(TokenStream2, bool)> {
        Ok(match &t.kind {
            Kind::Boolean(_) => (quote!(bool), false),
            // Every SQLite integer is 64 bits wide.
            Kind::Integer(_) => (quote!(i64), false),
            Kind::Float(_) => (quote!(f64), false),
            Kind::Text(_) => (quote!(::std::string::String), false),
            Kind::Enum(e) => self.sqlite(&e.ty, label)?,
            Kind::Bytes(_) => (quote!(::std::vec::Vec<u8>), false),
            Kind::Json(j) => (self.json_column(j, label)?, false),
            Kind::Union(_) => match t.label.as_deref() {
                Some(declared) if is_bool(declared) => (quote!(bool), false),
                Some(declared) => match affinity(declared) {
                    Affinity::Integer => (quote!(i64), false),
                    Affinity::Real => (quote!(f64), false),
                    Affinity::Text => (quote!(::std::string::String), false),
                    Affinity::Blob => (quote!(::std::vec::Vec<u8>), false),
                    Affinity::Numeric => return Err(syn::Error::new(self.span, format!("{label} is declared {declared}, which holds an integer, a real or text: cast it in the statement, as `cast({label} as real)`"))),
                },
                None => return Err(self.any_value(label)),
            },
            Kind::Unknown => return Err(self.any_value(label)),
            _ => return Err(self.unsupported(label, t, "")),
        })
    }

    /// A MySQL type's Rust type, as sqlx's MySQL driver decodes it on the session it opens, whose
    /// time zone is UTC, so a TIMESTAMP is an instant read exactly.
    fn mysql(&mut self, t: &Type, label: &str) -> syn::Result<(TokenStream2, bool)> {
        let name = t.label.as_deref().unwrap_or_default();
        Ok(match &t.kind {
            Kind::Boolean(_) => (quote!(bool), false),
            // MySQL's BOOLEAN is a TINYINT(1), which can hold 5 and which every MySQL client reads as a truth value.
            Kind::Integer(_) if name == "tinyint(1)" => (quote!(bool), false),
            // A YEAR is 0 or 1901 to 2155, which sqlx reads only into an unsigned integer.
            Kind::Integer(_) if name == "year" => (quote!(u16), false),
            // sqlx reads an UNSIGNED column only into an unsigned integer, and a MEDIUMINT into 32 bits.
            Kind::Integer(i) => match (i.bits, i.unsigned) {
                (8, false) => (quote!(i8), false),
                (16, false) => (quote!(i16), false),
                (24 | 32, false) => (quote!(i32), false),
                (64, false) => (quote!(i64), false),
                (8, true) => (quote!(u8), false),
                (16, true) => (quote!(u16), false),
                (24 | 32, true) => (quote!(u32), false),
                (64, true) => (quote!(u64), false),
                _ => return Err(self.unsupported(label, t, "")),
            },
            Kind::Float(f) => (float(f.exponent_bits, f.mantissa_bits), false),
            Kind::Decimal(_) => (self.decimal(label)?, false),
            // A SET is its elements joined by commas, as MySQL sends them. Text is read through
            // `reylo::Exact`, which reads it whatever its collation, as sqlx 0.8 does not.
            Kind::Text(_) | Kind::Enum(_) => (quote!(::std::string::String), true),
            Kind::Array(_) if name == "set" => (quote!(::std::string::String), true),
            Kind::Bytes(_) => (quote!(::std::vec::Vec<u8>), false),
            // A BIT(1) is a truth value, and a wider BIT its bits, as sqlx reads them.
            Kind::Array(a) if name == "bit" => match a.size.as_ref().and_then(|s| s.exact) {
                Some(1) => (quote!(bool), false),
                _ => (quote!(u64), false),
            },
            Kind::Json(j) => (self.json_column(j, label)?, false),
            Kind::Date(_) => (self.chrono_or_time(label, quote!(NaiveDate), quote!(Date))?, false),
            Kind::DateTime(_) => (self.chrono_or_time(label, quote!(NaiveDateTime), quote!(PrimitiveDateTime))?, false),
            Kind::Instant(_) => (self.chrono_or_time(label, quote!(DateTime<::reylo::sqlx::types::chrono::Utc>), quote!(OffsetDateTime))?, false),
            // A TIME is a signed duration up to 838 hours.
            Kind::Duration(_) => {
                self.serialize = false;
                (quote!(::reylo::sqlx::mysql::types::MySqlTime), false)
            }
            // MySQL's `:n + 1` is a BIGINT, a DOUBLE or a DECIMAL by what is bound.
            Kind::Union(_) => return Err(syn::Error::new(self.span, format!("{label} is one of several types, by what is bound: cast the expression in the statement, as `cast(:n + 1 as signed)`"))),
            Kind::Unknown => return Err(self.any_value(label)),
            _ => return Err(self.unsupported(label, t, "")),
        })
    }

    /// A Postgres enum type, as against text whose values a CHECK or a constant lists, or a
    /// domain over text: an enum type is named by its schema and is no domain.
    fn is_postgres_enum(&self, t: &Type) -> bool {
        matches!(&t.kind, Kind::Enum(_))
            && self.dialect == Dialect::Postgres
            && t.label.as_deref().is_some_and(|n| n.contains('.') && !self.domains.contains_key(n))
    }

    /// An item's stand-in name, until [`Types::finish`] names it.
    fn name(&mut self, base: String, fallback: String) -> Ident {
        let stand_in = format_ident!("__reylo_item_{}", self.named.len());
        self.named.push(Named { stand_in: stand_in.clone(), base, fallback });
        stand_in
    }

    /// A Postgres enum as a Rust enum, a variant per value, which sqlx decodes from the value's
    /// text and encodes to it; the type's name finds its oid at run time.
    fn postgres_enum(&mut self, t: &Type) -> syn::Result<EnumRef> {
        let name = t.label.clone().unwrap_or_default();
        if let Some(reference) = self.enums.get(&format!("type:{name}")) {
            return Ok(reference.clone());
        }
        let Kind::Enum(e) = &t.kind else { unreachable!("is_postgres_enum checked the kind") };
        if e.values.is_empty() {
            return Err(syn::Error::new(self.span, format!("the enum {name} has no values")));
        }
        let (schema, short) = name.rsplit_once('.').unwrap_or(("", &name));
        let base = names::type_name(short);
        let ident = self.name(base.clone(), format!("{}{base}", names::type_name(schema)));
        let schema_module = self.shared.then(|| module_name(schema)).flatten();
        match schema_module {
            Some(module) => {
                let shared_name = format_ident!("{}", base);
                let module_ident = syn::parse_str::<Ident>(&module).map_err(|e| syn::Error::new(self.span, e.to_string()))?;
                self.items.push(quote!(pub use __reylo_shared_root::#module_ident::#shared_name as #ident;));
                if !self.shared_enums.iter().any(|s| s.qualified == name) {
                    self.shared_enums.push(SharedEnum { qualified: name.clone(), schema: module, name: base, tokens: postgres_enum_tokens(&shared_name, &name, &e.values) });
                }
            }
            None => self.items.push(postgres_enum_tokens(&ident, &name, &e.values)),
        }
        let reference = EnumRef { ident, labels: e.values.clone() };
        self.enums.insert(format!("type:{name}"), reference.clone());
        Ok(reference)
    }

    /// A list of values as a Rust enum named after the column or parameter it first types, read
    /// from text and bound as text. Two that list the same values in the same order are one enum.
    fn list_enum(&mut self, values: &[String], label: &str) -> EnumRef {
        let key = format!("list:{}", values.join("\u{0}"));
        if let Some(reference) = self.enums.get(&key) {
            return reference.clone();
        }
        let base = names::type_name(label);
        let ident = self.name(base.clone(), format!("{base}Values"));
        self.items.push(enum_tokens(&ident, "", values, quote!()));
        let reference = EnumRef { ident, labels: values.to_vec() };
        self.enums.insert(key, reference.clone());
        reference
    }

    /// A JSON column: the shape the statement built, field by field, or the values the JSON Schema
    /// a CHECK holds it to admits, else any JSON.
    fn json_column(&mut self, j: &reylo_engine_core::types::JsonType, label: &str) -> syn::Result<TokenStream2> {
        let ty = match (&j.content, &j.schema) {
            (Some(content), _) => Some(self.json_value(&content.ty, content.nullable, label)?),
            (None, Some(schema)) => self.schema_value(schema, label)?,
            (None, None) => None,
        };
        Ok(match ty {
            Some(ty) => quote!(::reylo::sqlx::types::Json<#ty>),
            None => quote!(::reylo::serde_json::Value),
        })
    }

    /// The Rust type of the values a canonical JSON Schema admits (the vocabulary
    /// `schema_type::type_schema` writes), as serde reads them, `Option` where one may be null:
    /// `None` where only `serde_json::Value` holds them all, as a union of several kinds or a
    /// tuple does.
    fn schema_value(&mut self, schema: &serde_json::Value, label: &str) -> syn::Result<Option<TokenStream2>> {
        Ok(self.schema_parts(schema, label)?.map(|(ty, null)| if null { quote!(::core::option::Option<#ty>) } else { ty }))
    }

    /// [`Types::schema_value`] as its type and whether the value may be null.
    fn schema_parts(&mut self, schema: &serde_json::Value, label: &str) -> syn::Result<Option<(TokenStream2, bool)>> {
        let mut members = Vec::new();
        let mut null = false;
        schema_members(schema, &mut members, &mut null);
        let [member] = members.as_slice() else { return Ok(None) };
        let ty = match member {
            SchemaMember::Any => None,
            SchemaMember::Literals(values) => {
                if values.iter().all(|v| v.is_string()) {
                    Some(quote!(::std::string::String))
                } else if values.iter().all(|v| v.is_i64()) {
                    Some(quote!(i64))
                } else if values.iter().all(|v| v.is_number()) {
                    Some(quote!(f64))
                } else if values.iter().all(|v| v.is_boolean()) {
                    Some(quote!(bool))
                } else {
                    None
                }
            }
            SchemaMember::Kind("string", _) => Some(quote!(::std::string::String)),
            SchemaMember::Kind("integer", _) => Some(quote!(i64)),
            SchemaMember::Kind("number", _) => Some(quote!(f64)),
            SchemaMember::Kind("boolean", _) => Some(quote!(bool)),
            // A tuple has no one element type.
            SchemaMember::Kind("array", m) if m.contains_key("prefixItems") => None,
            SchemaMember::Kind("array", m) => {
                let element = match m.get("items") {
                    Some(items) => self.schema_value(items, &format!("{label}_item"))?,
                    None => None,
                };
                let element = element.unwrap_or_else(|| quote!(::reylo::serde_json::Value));
                Some(quote!(::std::vec::Vec<#element>))
            }
            SchemaMember::Kind("object", m) => self.schema_object(m, label)?,
            SchemaMember::Kind(..) => None,
        };
        Ok(ty.map(|ty| (ty, null)))
    }

    /// An object a JSON Schema describes: a struct of its properties, each one the object may
    /// leave out an `Option`; with the keys it doesn't name, where it admits others, kept in
    /// `additional_properties`, so that a value read and written back loses none. One that names
    /// no property is a map.
    fn schema_object(&mut self, m: &serde_json::Map<String, serde_json::Value>, label: &str) -> syn::Result<Option<TokenStream2>> {
        let empty = serde_json::Map::new();
        let properties = m.get("properties").and_then(|p| p.as_object()).unwrap_or(&empty);
        let required: BTreeSet<&str> = m.get("required").and_then(|r| r.as_array()).into_iter().flatten().filter_map(|k| k.as_str()).collect();
        let additional = m.get("additionalProperties").unwrap_or(&serde_json::Value::Bool(true));
        let closed = *additional == serde_json::Value::Bool(false);
        if properties.is_empty() {
            let value = match additional {
                serde_json::Value::Bool(_) => None,
                schema => self.schema_value(schema, &format!("{label}_value"))?,
            };
            let value = value.unwrap_or_else(|| quote!(::reylo::serde_json::Value));
            return Ok(Some(quote!(::std::collections::BTreeMap<::std::string::String, #value>)));
        }
        let mut idents = BTreeSet::new();
        let mut fields = Vec::new();
        for (key, property) in properties {
            // A key that is no Rust name, or two that read as one, leave the object untyped.
            let Some(field) = json_key_ident(key) else { return Ok(None) };
            if !idents.insert(field.to_string()) {
                return Ok(None);
            }
            let (inner, null) = self.schema_parts(property, &format!("{label}_{key}"))?.unwrap_or_else(|| (quote!(::reylo::serde_json::Value), false));
            fields.push(if required.contains(key.as_str()) {
                let ty = if null { quote!(::core::option::Option<#inner>) } else { inner };
                quote!(#[serde(rename = #key)] pub #field: #ty)
            } else {
                // A key the object may leave out reads as None, and None leaves it out.
                quote!(#[serde(rename = #key, default, skip_serializing_if = "::core::option::Option::is_none")] pub #field: ::core::option::Option<#inner>)
            });
        }
        if !closed {
            let mut rest = String::from("additional_properties");
            while idents.contains(&rest) {
                rest.push('_');
            }
            let rest = format_ident!("{}", rest);
            fields.push(quote!(#[serde(flatten)] pub #rest: ::reylo::serde_json::Map<::std::string::String, ::reylo::serde_json::Value>));
        }
        let base = names::type_name(label);
        let ident = self.name(base.clone(), format!("{base}Object"));
        self.items.push(quote!(
            #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq, ::reylo::serde::Serialize, ::reylo::serde::Deserialize)]
            #[serde(crate = "::reylo::serde")]
            pub struct #ident { #(#fields,)* }
        ));
        Ok(Some(quote!(#ident)))
    }

    /// A value inside a JSON document, typed by the SQL value the database wrote it from, as serde
    /// reads the JSON: a number as the integer or float it is, and a date, a time, a uuid or bytes
    /// as the text the database writes for it.
    fn json_value(&mut self, t: &Type, nullable: bool, label: &str) -> syn::Result<TokenStream2> {
        let ty = match &t.kind {
            Kind::Boolean(_) => quote!(bool),
            Kind::Integer(i) => match (i.bits, i.unsigned) {
                (b, false) if b <= 8 => quote!(i8),
                (b, false) if b <= 16 => quote!(i16),
                (b, false) if b <= 32 => quote!(i32),
                (b, false) if b <= 64 => quote!(i64),
                (b, true) if b <= 8 => quote!(u8),
                (b, true) if b <= 16 => quote!(u16),
                (b, true) if b <= 32 => quote!(u32),
                (b, true) if b <= 64 => quote!(u64),
                _ => quote!(::reylo::serde_json::Number),
            },
            Kind::Float(_) => quote!(f64),
            // serde_json reads a number with a fraction as an f64, which rounds a decimal.
            Kind::Decimal(_) => return Err(syn::Error::new(self.span, format!("{label} holds a decimal inside JSON, which no Rust number reads exactly: cast it to text inside the JSON, as `x::text`"))),
            Kind::Text(_) | Kind::Uuid(_) | Kind::Bytes(_) | Kind::Date(_) | Kind::Time(_) | Kind::DateTime(_) | Kind::Instant(_) | Kind::ZonedDateTime(_) | Kind::Duration(_) => quote!(::std::string::String),
            Kind::Enum(e) => return self.json_value(&e.ty, nullable, label),
            Kind::Array(a) => {
                let element = self.json_value(&a.element.ty, a.element.nullable, &format!("{label}_item"))?;
                quote!(::std::vec::Vec<#element>)
            }
            Kind::Struct(r) => {
                let base = names::type_name(label);
                let ident = self.name(base.clone(), format!("{base}Object"));
                let mut fields = Vec::new();
                for f in &r.fields {
                    let key = f.name.clone();
                    let field = crate::field_ident(&key, self.span)?;
                    let inner = self.json_value(&f.ty, f.nullable || f.optional, &format!("{label}_{key}"))?;
                    // A key the object may leave out reads as None.
                    let default = f.optional.then(|| quote!(#[serde(default)]));
                    fields.push(quote!(#default #[serde(rename = #key)] pub #field: #inner));
                }
                self.items.push(quote!(
                    #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq, ::reylo::serde::Serialize, ::reylo::serde::Deserialize)]
                    #[serde(crate = "::reylo::serde")]
                    pub struct #ident { #(#fields,)* }
                ));
                quote!(#ident)
            }
            Kind::Json(inner) => match (&inner.content, &inner.schema) {
                (Some(content), _) => self.json_value(&content.ty, content.nullable, label)?,
                (None, Some(schema)) => self.schema_value(schema, label)?.unwrap_or_else(|| quote!(::reylo::serde_json::Value)),
                (None, None) => quote!(::reylo::serde_json::Value),
            },
            _ => quote!(::reylo::serde_json::Value),
        };
        Ok(if nullable { quote!(::core::option::Option<#ty>) } else { ty })
    }

    fn decimal(&self, label: &str) -> syn::Result<TokenStream2> {
        match self.options.decimals {
            Some(Decimals::RustDecimal) => Ok(quote!(::reylo::sqlx::types::Decimal)),
            Some(Decimals::BigDecimal) => Ok(quote!(::reylo::sqlx::types::BigDecimal)),
            None => Err(syn::Error::new(self.span, format!("{label} is a decimal: enable reylo's rust_decimal or bigdecimal feature"))),
        }
    }

    fn uuid(&self, label: &str) -> syn::Result<TokenStream2> {
        if self.options.uuid {
            Ok(quote!(::reylo::sqlx::types::Uuid))
        } else {
            Err(syn::Error::new(self.span, format!("{label} is a uuid: enable reylo's uuid feature")))
        }
    }

    fn chrono_or_time(&self, label: &str, chrono: TokenStream2, time: TokenStream2) -> syn::Result<TokenStream2> {
        match self.options.dates {
            Some(Dates::Chrono) => Ok(quote!(::reylo::sqlx::types::chrono::#chrono)),
            Some(Dates::Time) => Ok(quote!(::reylo::sqlx::types::time::#time)),
            None => Err(syn::Error::new(self.span, format!("{label} is a date or time: enable reylo's chrono or time feature"))),
        }
    }

    fn any_value(&self, label: &str) -> syn::Error {
        syn::Error::new(self.span, format!("{label} may hold a value of any type: cast it in the statement, as `cast({label} as integer)`"))
    }

    fn unsupported(&self, label: &str, t: &Type, hint: &str) -> syn::Error {
        let what = t.label.clone().unwrap_or_else(|| t.short());
        let hint = if hint.is_empty() { String::new() } else { format!(": {hint}") };
        syn::Error::new(self.span, format!("{label}: Reylo has no Rust type for {what} yet{hint}"))
    }
}

/// One kind of value a canonical JSON Schema admits, besides null.
enum SchemaMember<'a> {
    /// A `type`, with the keywords beside it (`items`, `properties`, …).
    Kind(&'a str, &'a serde_json::Map<String, serde_json::Value>),
    /// An `enum`'s values.
    Literals(Vec<&'a serde_json::Value>),
    /// Any value, as `true` or `{}` admit.
    Any,
}

/// The kinds `schema` admits, an `anyOf`'s and a `type` list's each one, into `members`, and
/// whether it admits null into `null`.
fn schema_members<'a>(schema: &'a serde_json::Value, members: &mut Vec<SchemaMember<'a>>, null: &mut bool) {
    let serde_json::Value::Object(m) = schema else {
        // `false` admits nothing, and `true` anything.
        if *schema != serde_json::Value::Bool(false) {
            members.push(SchemaMember::Any);
        }
        return;
    };
    if let Some(serde_json::Value::Array(all)) = m.get("anyOf") {
        for s in all {
            schema_members(s, members, null);
        }
        return;
    }
    if let Some(serde_json::Value::Array(values)) = m.get("enum") {
        *null |= values.iter().any(|v| v.is_null());
        let values: Vec<_> = values.iter().filter(|v| !v.is_null()).collect();
        if !values.is_empty() {
            members.push(SchemaMember::Literals(values));
        }
        return;
    }
    let kinds: Vec<&str> = match m.get("type") {
        Some(serde_json::Value::String(kind)) => vec![kind],
        Some(serde_json::Value::Array(kinds)) => kinds.iter().filter_map(|k| k.as_str()).collect(),
        _ => Vec::new(),
    };
    if kinds.is_empty() {
        members.push(SchemaMember::Any);
    }
    for kind in kinds {
        if kind == "null" {
            *null = true;
        } else {
            members.push(SchemaMember::Kind(kind, m));
        }
    }
}

/// A JSON object's key as a struct field: the key itself where it is a Rust name, else with each
/// character no Rust name holds written `_` (`font-size` as `font_size`); `None` for a key that
/// leaves nothing to name.
fn json_key_ident(key: &str) -> Option<Ident> {
    let mut name: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    if name.is_empty() || name.chars().all(|c| c == '_') || matches!(name.as_str(), "self" | "Self" | "super" | "crate") {
        return None;
    }
    Some(syn::parse_str::<Ident>(&name).unwrap_or_else(|_| Ident::new_raw(&name, Span::call_site())))
}

/// An enum of `values` named `ident`, with `reylo::Enum` (of the Postgres type `type_name`, or of
/// no type for a list), an inherent `label`, serde's `Serialize` as its value, and `more`.
pub fn enum_tokens(ident: &Ident, type_name: &str, values: &[String], more: TokenStream2) -> TokenStream2 {
    let variants: Vec<Ident> = names::variants(values).iter().map(|v| format_ident!("{}", v)).collect();
    let trait_impl = enum_impl(ident, type_name, values);
    quote!(
        #[allow(non_camel_case_types)]
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash)]
        pub enum #ident { #(#variants,)* }
        impl #ident {
            /// The value the database stores.
            #[allow(dead_code)]
            pub const fn label(self) -> &'static str {
                match self { #(#ident::#variants => #values,)* }
            }
        }
        #trait_impl
        impl ::reylo::serde::Serialize for #ident {
            fn serialize<S: ::reylo::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_str(::reylo::Enum::label(*self))
            }
        }
        #more
    )
}

/// `reylo::Enum` for the enum `ident` of `values`, whose variants are named by [`names::variants`],
/// and `reylo::HasLabel` for each value, so that a list's value casts into it. A Postgres enum
/// type's `Id` is the type (`reylo::EnumId`), and a list's its values (`reylo::Labels`).
pub fn enum_impl(ident: &Ident, type_name: &str, values: &[String]) -> TokenStream2 {
    let variants: Vec<Ident> = names::variants(values).iter().map(|v| format_ident!("{}", v)).collect();
    let id = if type_name.is_empty() {
        let labels = values.iter().rev().fold(quote!(()), |rest, v| quote!((::reylo::Label<{ ::reylo::__private::label_hash(#v) }>, #rest)));
        quote!(::reylo::Labels<#labels>)
    } else {
        let hash = proc_macro2::Literal::u64_suffixed(enum_hash(type_name, values));
        quote!(::reylo::EnumId<#hash>)
    };
    quote!(
        #(impl ::reylo::HasLabel<{ ::reylo::__private::label_hash(#values) }> for #ident {
            const VARIANT: Self = #ident::#variants;
        })*
        impl ::reylo::Enum for #ident {
            type Id = #id;
            const TYPE: &'static str = #type_name;
            const LABELS: &'static [&'static str] = &[#(#values),*];
            fn label(self) -> &'static str {
                match self { #(#ident::#variants => #values,)* }
            }
            fn from_label(label: &str) -> ::core::option::Option<Self> {
                match label {
                    #(#values => ::core::option::Option::Some(#ident::#variants),)*
                    _ => ::core::option::Option::None,
                }
            }
        }
    )
}

/// FNV-1a over the type's name and each value, each ended by a zero byte: the same for every Rust
/// enum of one Postgres enum type, so it names the type as a type (`reylo::EnumId`).
pub fn enum_hash(type_name: &str, values: &[String]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in std::iter::once(type_name).chain(values.iter().map(String::as_str)) {
        for byte in part.bytes().chain([0]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

/// A Postgres enum type `name` as the Rust enum `ident`, with sqlx's impls, which find the type
/// by its name at run time.
pub fn postgres_enum_tokens(ident: &Ident, name: &str, values: &[String]) -> TokenStream2 {
    enum_tokens(ident, name, values, postgres_impls(ident, name))
}

/// sqlx's impls for the enum `ident` of the Postgres enum type `name`: decoded from its value's
/// text, encoded as it, and the type found by its name at run time.
pub fn postgres_impls(ident: &Ident, name: &str) -> TokenStream2 {
    let (schema, short) = name.rsplit_once('.').unwrap_or(("", name));
    let array = if schema.is_empty() { format!("_{short}") } else { format!("{schema}._{short}") };
    let message = format!("is no value of {name}");
    quote!(
        impl ::reylo::sqlx::Type<::reylo::sqlx::Postgres> for #ident {
            fn type_info() -> ::reylo::sqlx::postgres::PgTypeInfo {
                ::reylo::sqlx::postgres::PgTypeInfo::with_name(#name)
            }
            fn compatible(ty: &::reylo::sqlx::postgres::PgTypeInfo) -> bool {
                let found = ::reylo::sqlx::TypeInfo::name(ty);
                found == #short || found == #name
            }
        }
        impl ::reylo::sqlx::postgres::PgHasArrayType for #ident {
            fn array_type_info() -> ::reylo::sqlx::postgres::PgTypeInfo {
                ::reylo::sqlx::postgres::PgTypeInfo::with_name(#array)
            }
        }
        impl<'r> ::reylo::sqlx::Decode<'r, ::reylo::sqlx::Postgres> for #ident {
            fn decode(value: ::reylo::sqlx::postgres::PgValueRef<'r>) -> ::core::result::Result<Self, ::reylo::sqlx::error::BoxDynError> {
                let text = <&str as ::reylo::sqlx::Decode<'r, ::reylo::sqlx::Postgres>>::decode(value)?;
                <#ident as ::reylo::Enum>::from_label(text).ok_or_else(|| ::std::format!("{text:?} {}", #message).into())
            }
        }
        impl<'q> ::reylo::sqlx::Encode<'q, ::reylo::sqlx::Postgres> for #ident {
            fn encode_by_ref(&self, buf: &mut ::reylo::sqlx::postgres::PgArgumentBuffer) -> ::core::result::Result<::reylo::sqlx::encode::IsNull, ::reylo::sqlx::error::BoxDynError> {
                <&str as ::reylo::sqlx::Encode<'q, ::reylo::sqlx::Postgres>>::encode_by_ref(&::reylo::Enum::label(*self), buf)
            }
        }
    )
}

/// sqlx's impls for the enum `ident` of a list of text values on `db` (`Postgres`, `MySql` or
/// `Sqlite`): decoded from text, a value outside the list an error, and encoded as text.
pub fn text_impls(ident: &Ident, db: &TokenStream2) -> TokenStream2 {
    quote!(
        impl ::reylo::sqlx::Type<::reylo::sqlx::#db> for #ident {
            fn type_info() -> <::reylo::sqlx::#db as ::reylo::sqlx::Database>::TypeInfo {
                <str as ::reylo::sqlx::Type<::reylo::sqlx::#db>>::type_info()
            }
            fn compatible(ty: &<::reylo::sqlx::#db as ::reylo::sqlx::Database>::TypeInfo) -> bool {
                <str as ::reylo::sqlx::Type<::reylo::sqlx::#db>>::compatible(ty)
            }
        }
        impl<'r> ::reylo::sqlx::Decode<'r, ::reylo::sqlx::#db> for #ident {
            fn decode(value: <::reylo::sqlx::#db as ::reylo::sqlx::Database>::ValueRef<'r>) -> ::core::result::Result<Self, ::reylo::sqlx::error::BoxDynError> {
                let text = <&str as ::reylo::sqlx::Decode<'r, ::reylo::sqlx::#db>>::decode(value)?;
                <#ident as ::reylo::Enum>::from_label(text).ok_or_else(|| ::std::format!("{text:?} is none of the values {}", <#ident as ::reylo::Enum>::LABELS.join(", ")).into())
            }
        }
        impl<'q> ::reylo::sqlx::Encode<'q, ::reylo::sqlx::#db> for #ident {
            fn encode_by_ref(&self, buf: &mut ::reylo::__private::ArgumentBuffer<'q, ::reylo::sqlx::#db>) -> ::core::result::Result<::reylo::sqlx::encode::IsNull, ::reylo::sqlx::error::BoxDynError> {
                <&str as ::reylo::sqlx::Encode<'q, ::reylo::sqlx::#db>>::encode_by_ref(&::reylo::Enum::label(*self), buf)
            }
        }
    )
}

/// A schema's name as a Rust module's, written raw where it is a keyword; none where it is no name.
fn module_name(schema: &str) -> Option<String> {
    let valid = !schema.is_empty() && schema.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !schema.starts_with(|c: char| c.is_ascii_digit()) && !matches!(schema, "self" | "Self" | "super" | "crate" | "_");
    valid.then(|| if syn::parse_str::<Ident>(schema).is_ok() { schema.to_string() } else { format!("r#{schema}") })
}

/// The smallest Rust float whose exponent and mantissa are both at least these widths.
fn float(exponent_bits: u32, mantissa_bits: u32) -> TokenStream2 {
    if exponent_bits <= 8 && mantissa_bits <= 23 {
        quote!(f32)
    } else {
        quote!(f64)
    }
}

/// A type's name without its schema.
fn bare(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

#[derive(PartialEq, Eq)]
enum Affinity {
    Integer,
    Text,
    Blob,
    Real,
    Numeric,
}

/// SQLite's affinity of a declared type, by its five rules (`sqliteAffinity` in `src/values.ts`).
fn affinity(declared: &str) -> Affinity {
    let d = declared.to_ascii_uppercase();
    if d.contains("INT") {
        Affinity::Integer
    } else if d.contains("CHAR") || d.contains("CLOB") || d.contains("TEXT") {
        Affinity::Text
    } else if d.contains("BLOB") || d.is_empty() {
        Affinity::Blob
    } else if d.contains("REAL") || d.contains("FLOA") || d.contains("DOUB") {
        Affinity::Real
    } else {
        Affinity::Numeric
    }
}

/// A SQLite column declared BOOLEAN, which the loader reads as a truth value too.
fn is_bool(declared: &str) -> bool {
    let d = declared.trim().to_ascii_lowercase();
    d == "bool" || d == "boolean"
}

#[cfg(test)]
mod tests {
    use super::*;
    use reylo_engine_core::types::JsonType;
    use serde_json::json;

    /// A Postgres jsonb column a CHECK holds to `schema` (in the canonical vocabulary the engine
    /// writes), as its Rust type and the items it declares, each as Rust source.
    fn checked(schema: serde_json::Value) -> (String, Vec<String>) {
        let t = Type { kind: Kind::Json(JsonType { content: None, schema: Some(schema) }), label: Some("jsonb".into()), comment: None, title: None };
        let mut types = Types::new(Dialect::Postgres, Options::default(), BTreeMap::new(), Span::call_site(), false);
        let shape = types.column(&t, false, "prefs").unwrap();
        let finished = types.finish(&BTreeSet::new()).unwrap();
        let normal = |t: TokenStream2| t.to_string().replace(' ', "");
        (normal(finished.named(shape.ty())), finished.items.into_iter().map(normal).collect())
    }

    #[test]
    fn a_checked_object_reads_as_a_struct_of_its_properties() {
        let (ty, items) = checked(json!({
            "type": "object",
            "properties": { "theme": { "enum": ["dark", "light"] }, "font_size": { "type": "integer" }, "tab-width": { "type": ["integer", "null"] } },
            "required": ["theme", "font_size"],
            "additionalProperties": false,
        }));
        assert_eq!(ty, "::reylo::sqlx::types::Json<Prefs>");
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert!(item.contains("pubstructPrefs{"), "{item}");
        assert!(item.contains("#[serde(rename=\"theme\")]pubtheme:::std::string::String"), "{item}");
        assert!(item.contains("#[serde(rename=\"font_size\")]pubfont_size:i64"), "{item}");
        // Left out, or null: the one Option either way.
        assert!(item.contains("#[serde(rename=\"tab-width\",default,skip_serializing_if=\"::core::option::Option::is_none\")]pubtab_width:::core::option::Option<i64>"), "{item}");
        assert!(!item.contains("additional_properties"), "{item}");
    }

    #[test]
    fn an_object_that_admits_other_keys_keeps_them() {
        let (_, items) = checked(json!({ "type": "object", "properties": { "a": { "type": "string" } }, "required": ["a"], "additionalProperties": true }));
        assert!(items[0].contains("#[serde(flatten)]pubadditional_properties:::reylo::serde_json::Map<::std::string::String,::reylo::serde_json::Value>"), "{}", items[0]);
    }

    #[test]
    fn arrays_maps_and_nullable_values() {
        assert_eq!(checked(json!({ "type": "array", "items": { "type": "string" } })).0, "::reylo::sqlx::types::Json<::std::vec::Vec<::std::string::String>>");
        assert_eq!(checked(json!({ "type": "object", "additionalProperties": { "type": "number" } })).0, "::reylo::sqlx::types::Json<::std::collections::BTreeMap<::std::string::String,f64>>");
        assert_eq!(checked(json!({ "type": ["boolean", "null"] })).0, "::reylo::sqlx::types::Json<::core::option::Option<bool>>");
        assert_eq!(checked(json!({ "anyOf": [{ "type": "object", "properties": { "x": { "type": "integer" } }, "required": ["x"], "additionalProperties": false }, { "type": "null" }] })).0, "::reylo::sqlx::types::Json<::core::option::Option<Prefs>>");
    }

    #[test]
    fn what_no_one_rust_type_holds_stays_any_json() {
        for schema in [json!({}), json!({ "type": ["string", "integer"] }), json!({ "type": "array", "prefixItems": [{ "type": "string" }] }), json!({ "enum": ["a", 1] })] {
            assert_eq!(checked(schema.clone()).0, "::reylo::serde_json::Value", "{schema}");
        }
    }
}
