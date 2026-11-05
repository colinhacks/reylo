//! The vocabulary the Postgres describe tier answers in, beside the type model every database
//! shares ([`reylo_engine_core::types`]).

pub use reylo_engine_core::types::*;
use serde::{Deserialize, Serialize};

/// One described result column: the server's type and its nullability as the engine settled it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DescribedColumn {
    pub name: String,
    pub type_oid: u32,
    /// The type modifier the describe reports for it (`varchar(20)`); -1, none, where it has none.
    #[serde(default = "crate::pgtypes::no_typmod", skip_serializing_if = "crate::pgtypes::is_no_typmod")]
    pub typmod: i32,
    pub type_name: String,
    pub nullable: bool,
    /// For an array column the statement builds (`ARRAY[a, b]`, `array_agg(x)`, an array
    /// literal), whether an element may be NULL; `None` for every other column, a table's array
    /// column and a function's array result among them ([`Type::Array`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nullable_items: Option<bool>,
    /// For an array column the statement builds of arrays (`array_agg(tags)`), the dimensions it
    /// adds over the type the describe reports ([`crate::infer::Nesting`]); `None` for every other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested: Option<crate::infer::Nesting>,
    /// For a bare read of a string column a CHECK keeps to a list (`status IN ('active',
    /// 'banned')`), those strings: its values are a union of literals, as an enum's labels are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    /// For a bare read of a `json` or `jsonb` column a CHECK holds to a JSON Schema the database
    /// validates (`jsonb_matches_schema('…', doc)`), the canonical schema of the values it admits
    /// ([`crate::schema_type`]): its values are those, as [`Type::Json`]'s `schema` says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<serde_json::Value>,
    /// For a bare read of a float or `numeric` column, the non-finite values a CHECK keeps out of
    /// it (`price < 1e9` keeps out NaN and infinity): its type holds those no more.
    #[serde(default, skip_serializing_if = "NonFinite::is_none")]
    pub non_finite: NonFinite,
    /// Whether the row value the column holds, or each one of an array of them, is a stored row
    /// of the table whose row type it is: a whole row the statement reads (`d` of `from docs d`,
    /// through a view, a derived table or a WITH query too), or one a scalar or `ARRAY` subquery
    /// returns (`array(select c from child c …)`). Such a row met the table's `NOT NULL` and
    /// CHECKs; a row value the statement builds (`row(…)::docs`), a function returns or another
    /// table's column holds only shares the type.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub whole_row: bool,
}

/// Which of the non-finite values a float or `numeric` column's CHECKs keep out of it, each `true`
/// where one does ([`crate::postgres::check_non_finite`]). Every CHECK holds, so a value one of
/// them keeps out is kept out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonFinite {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nan: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub infinity: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub minus_infinity: bool,
}

impl NonFinite {
    pub fn is_none(&self) -> bool {
        *self == NonFinite::default()
    }

    /// What two CHECKs keep out together.
    pub fn and(self, other: NonFinite) -> NonFinite {
        NonFinite { nan: self.nan || other.nan, infinity: self.infinity || other.infinity, minus_infinity: self.minus_infinity || other.minus_infinity }
    }

    /// `ty` without the values kept out, where it is a float or a decimal. The type has one flag
    /// for both infinities, so it loses it only where both are kept out.
    pub fn apply(self, ty: Type) -> Type {
        let infinity = self.infinity && self.minus_infinity;
        match ty.kind {
            Kind::Float(f) => Type { kind: Kind::Float(FloatType { nan: f.nan && !self.nan, infinity: f.infinity && !infinity, ..f }), ..ty },
            Kind::Decimal(d) => Type { kind: Kind::Decimal(DecimalType { nan: d.nan && !self.nan, infinity: d.infinity && !infinity, ..d }), ..ty },
            kind => Type { kind, ..ty },
        }
    }
}

/// One described statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DescribedStatement {
    /// Parameter type OIDs in placeholder order ($1 first).
    pub param_oids: Vec<u32>,
    pub columns: Vec<DescribedColumn>,
}
