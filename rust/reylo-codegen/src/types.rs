//! A type descriptor's Rust type (`spec/type-descriptor.ts`, `engine/core/src/types.rs`).
//!
//! The kind and the fields it requires pick the Rust type, the database's name decides where a
//! client's convention does (a SQLite column declared BOOLEAN is a `bool`, as `columnKind` in
//! `src/values.ts` reads it), and the widening fields say which values the Rust type must hold.
//! Where the usual Rust type holds most of them and not all (a Postgres `date` may hold infinity,
//! and years chrono cannot write), the column is decoded through `reylo::Exact`, which refuses
//! what the type cannot hold when it is read; no value is ever changed. Where no Rust type holds
//! the values at all, the expansion fails at the statement and says how to cast it.

use crate::{Dates, Decimals, Dialect, Options};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use reylo_engine_core::types::{Kind, Type};
use std::collections::{BTreeMap, HashMap};

/// How a value is decoded into its Rust type: as sqlx decodes it, or, for a `Leaf` marked
/// `exact`, through `reylo::Exact`, inside any `Option` and `Vec` around it.
pub enum Shape {
    Leaf { ty: TokenStream2, exact: bool },
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
            Shape::Leaf { ty, exact: true } => quote!(::reylo::Exact<#ty>),
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

    fn exact(&self) -> bool {
        match self {
            Shape::Leaf { exact, .. } => *exact,
            Shape::Option(s) | Shape::Vec(s) => s.exact(),
        }
    }

    /// `decoded`, of the decode type, as the field's type.
    pub fn unwrap(&self, decoded: TokenStream2) -> TokenStream2 {
        if !self.exact() {
            return decoded;
        }
        match self {
            Shape::Leaf { .. } => quote!(#decoded.0),
            Shape::Option(s) => {
                let inner = s.unwrap(quote!(v));
                quote!(#decoded.map(|v| #inner))
            }
            Shape::Vec(s) => {
                let inner = s.unwrap(quote!(v));
                quote!(#decoded.into_iter().map(|v| #inner).collect::<::std::vec::Vec<_>>())
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    Column,
    Param,
}

/// The Rust types of one statement, and the items they need: an enum per Postgres enum type, a
/// struct per JSON object the statement builds.
pub struct Types {
    dialect: Dialect,
    options: Options,
    /// Each domain the statement's types are named by, with its base type's name.
    domains: BTreeMap<String, String>,
    span: Span,
    items: Vec<TokenStream2>,
    enums: HashMap<String, proc_macro2::Ident>,
    /// Whether every type written so far has a serde form, so the row can derive `Serialize`.
    pub serialize: bool,
}

impl Types {
    pub fn new(dialect: Dialect, options: Options, domains: BTreeMap<String, String>, span: Span) -> Types {
        Types { dialect, options, domains, span, items: Vec::new(), enums: HashMap::new(), serialize: true }
    }

    /// The enums and structs the expansion declares beside its `Row`.
    pub fn items(&self) -> &[TokenStream2] {
        &self.items
    }

    /// A column's Rust type, `Option` where it may be null.
    pub fn column(&mut self, t: &Type, nullable: bool, label: &str) -> syn::Result<Shape> {
        let shape = self.value(t, label, Use::Column)?;
        Ok(if nullable { Shape::Option(Box::new(shape)) } else { shape })
    }

    /// A parameter's Rust type, which sqlx encodes; the caller wraps a nullable one.
    pub fn param(&mut self, t: &Type, label: &str) -> syn::Result<TokenStream2> {
        Ok(self.value(t, label, Use::Param)?.ty())
    }

    /// Whether the type's values are a `String`, so that a parameter of it takes a `&str` too.
    pub fn is_string(&self, t: &Type) -> bool {
        match (&t.kind, self.dialect) {
            (Kind::Text(_), _) => true,
            // A CHECK's list or a MySQL ENUM is its base type's values; a Postgres enum type is a Rust enum.
            (Kind::Enum(e), _) => !self.is_enum(t) && self.is_string(&e.ty),
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
        let (ty, exact) = match self.dialect {
            Dialect::Postgres => self.postgres(t, label)?,
            Dialect::Sqlite => self.sqlite(t, label)?,
            Dialect::Mysql => self.mysql(t, label)?,
        };
        Ok(Shape::Leaf { ty, exact: exact && at == Use::Column })
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
            Kind::Decimal(_) => (self.decimal(label)?, false),
            Kind::Enum(_) if self.is_enum(t) => (self.postgres_enum(t)?, false),
            // A list a CHECK or a literal names: its base type's values.
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
            // An ENUM is its value, and a SET its elements joined by commas, as MySQL sends them.
            Kind::Text(_) | Kind::Enum(_) => (quote!(::std::string::String), false),
            Kind::Array(_) if name == "set" => (quote!(::std::string::String), false),
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
    fn is_enum(&self, t: &Type) -> bool {
        matches!(&t.kind, Kind::Enum(_))
            && self.dialect == Dialect::Postgres
            && t.label.as_deref().is_some_and(|n| n.contains('.') && !self.domains.contains_key(n))
    }

    /// A Postgres enum as a Rust enum, a variant per value, which sqlx decodes from the value's
    /// text and encodes to it; the type's name finds its oid at run time.
    fn postgres_enum(&mut self, t: &Type) -> syn::Result<TokenStream2> {
        let name = t.label.clone().unwrap_or_default();
        if let Some(ident) = self.enums.get(&name) {
            return Ok(quote!(#ident));
        }
        let Kind::Enum(e) = &t.kind else { unreachable!("is_enum checked the kind") };
        let values = e.values.clone();
        let (schema, short) = name.rsplit_once('.').unwrap_or(("", &name));
        let ident = format_ident!("{}", pascal(short));
        let mut variants: Vec<proc_macro2::Ident> = Vec::new();
        for value in &values {
            let mut variant = pascal(&value.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect::<String>());
            if variant.is_empty() || variant.starts_with(|c: char| c.is_ascii_digit()) {
                variant = format!("V{variant}");
            }
            while variants.iter().any(|v| *v == variant) {
                variant.push('_');
            }
            variants.push(format_ident!("{}", variant));
        }
        if variants.is_empty() {
            return Err(syn::Error::new(self.span, format!("the enum {name} has no values")));
        }
        let array = if schema.is_empty() { format!("_{short}") } else { format!("{schema}._{short}") };
        let short = short.to_string();
        let message = format!("is no value of {name}");
        self.items.push(quote!(
            #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash)]
            pub enum #ident { #(#variants,)* }
            impl #ident {
                /// The value the database stores.
                pub const fn label(self) -> &'static str {
                    match self { #(#ident::#variants => #values,)* }
                }
            }
            impl ::reylo::serde::Serialize for #ident {
                fn serialize<S: ::reylo::serde::Serializer>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error> {
                    serializer.serialize_str(self.label())
                }
            }
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
                    match <&str as ::reylo::sqlx::Decode<'r, ::reylo::sqlx::Postgres>>::decode(value)? {
                        #(#values => ::core::result::Result::Ok(#ident::#variants),)*
                        other => ::core::result::Result::Err(::std::format!("{other:?} {}", #message).into()),
                    }
                }
            }
            impl<'q> ::reylo::sqlx::Encode<'q, ::reylo::sqlx::Postgres> for #ident {
                fn encode_by_ref(&self, buf: &mut ::reylo::sqlx::postgres::PgArgumentBuffer) -> ::core::result::Result<::reylo::sqlx::encode::IsNull, ::reylo::sqlx::error::BoxDynError> {
                    <&str as ::reylo::sqlx::Encode<'q, ::reylo::sqlx::Postgres>>::encode_by_ref(&self.label(), buf)
                }
            }
        ));
        self.enums.insert(name, ident.clone());
        Ok(quote!(#ident))
    }

    /// A JSON column: the shape the statement built, field by field, else any JSON.
    fn json_column(&mut self, j: &reylo_engine_core::types::JsonType, label: &str) -> syn::Result<TokenStream2> {
        Ok(match &j.content {
            Some(content) => {
                let ty = self.json_value(&content.ty, content.nullable, label)?;
                quote!(::reylo::sqlx::types::Json<#ty>)
            }
            None => quote!(::reylo::serde_json::Value),
        })
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
                let ident = format_ident!("{}", pascal(label));
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
            Kind::Json(inner) => match &inner.content {
                Some(content) => self.json_value(&content.ty, content.nullable, label)?,
                None => quote!(::reylo::serde_json::Value),
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

fn pascal(name: &str) -> String {
    name.split('_').filter(|s| !s.is_empty()).map(|s| s[..1].to_uppercase() + &s[1..]).collect()
}
