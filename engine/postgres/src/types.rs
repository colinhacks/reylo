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
    /// For a bare read of a column, the bounds, the divisor and the pattern a CHECK holds its
    /// values to (`age between 0 and 150`, `qty % 5 = 0`, `email like '%@%'`).
    #[serde(default, skip_serializing_if = "CheckFacts::is_none")]
    pub facts: CheckFacts,
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

/// What a column's CHECKs hold its non-null values to ([`crate::postgres::check_facts`]). A bound is
/// its canonical text and whether it is exclusive. Every CHECK holds, so a value meets them all.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CheckFacts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<(String, bool)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<(String, bool)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiple_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Pattern>,
}

impl CheckFacts {
    pub fn is_none(&self) -> bool {
        *self == CheckFacts::default()
    }

    /// What two CHECKs hold a value to together: the tighter of each bound, and at equal values the
    /// exclusive one. The type has one divisor and one pattern, so the first of each stands; either
    /// is true of every value.
    pub fn and(self, other: CheckFacts) -> CheckFacts {
        let tighter = |a: Option<(String, bool)>, b: Option<(String, bool)>, above: bool| match (a, b) {
            (Some(a), Some(b)) => {
                let (x, y) = (a.0.parse::<f64>().unwrap_or(f64::NAN), b.0.parse::<f64>().unwrap_or(f64::NAN));
                Some(if x == y {
                    if a.1 { a } else { b }
                } else if (x > y) == above {
                    a
                } else {
                    b
                })
            }
            (a, b) => a.or(b),
        };
        CheckFacts { min: tighter(self.min, other.min, true), max: tighter(self.max, other.max, false), multiple_of: self.multiple_of.or(other.multiple_of), pattern: self.pattern.or(other.pattern) }
    }

    /// `ty` with these facts, where it is of a kind that holds them. A bound its width or its
    /// precision already implies is left out, as the format asks, and a CHECK no value can meet says
    /// nothing a reader could use.
    pub fn apply(&self, ty: Type) -> Type {
        let low = self.min.as_ref().map(|(v, _)| v.parse::<f64>().unwrap_or(f64::NEG_INFINITY));
        let high = self.max.as_ref().map(|(v, _)| v.parse::<f64>().unwrap_or(f64::INFINITY));
        if low.zip(high).is_some_and(|(l, h)| l > h) {
            return ty;
        }
        let Type { kind, name, comment } = ty;
        let kind = match kind {
            Kind::Integer(mut i) => {
                let lowest = if i.unsigned { 0 } else { -(1i128 << (i.bits.min(127) - 1)) };
                let highest = if i.unsigned { (1i128 << i.bits.min(126)) - 1 } else { (1i128 << (i.bits.min(127) - 1)) - 1 };
                let whole = |bound: &Option<(String, bool)>, step: i128| bound.as_ref().and_then(|(v, exclusive)| Some(v.parse::<i128>().ok()? + if *exclusive { step } else { 0 }));
                if let Some(min) = whole(&self.min, 1).filter(|min| *min > lowest) {
                    i.bounds.min = Some(min.to_string());
                }
                if let Some(max) = whole(&self.max, -1).filter(|max| *max < highest) {
                    i.bounds.max = Some(max.to_string());
                }
                i.multiple_of = self.multiple_of.clone().or(i.multiple_of);
                Kind::Integer(i)
            }
            Kind::Float(mut f) => {
                self.bound(&mut f.bounds, None);
                Kind::Float(f)
            }
            Kind::Decimal(mut d) => {
                // The values a precision and a scale admit are below 10^(precision - scale).
                let reach = d.precision.map(|p| 10f64.powi(p as i32 - d.max_scale.unwrap_or(0)));
                self.bound(&mut d.bounds, reach);
                d.multiple_of = self.multiple_of.clone().or(d.multiple_of);
                Kind::Decimal(d)
            }
            Kind::Text(mut t) if t.values.is_none() => {
                t.pattern = self.pattern.clone().or(t.pattern);
                Kind::Text(t)
            }
            kind => kind,
        };
        Type { kind, name, comment }
    }

    /// Each bound on `bounds`, in place of what that side held. `reach` is how far the type's own
    /// values go either way, which a bound past it says nothing beyond.
    fn bound(&self, bounds: &mut Bounds, reach: Option<f64>) {
        let inside = |v: &str| reach.is_none_or(|r| v.parse::<f64>().is_ok_and(|v| v.abs() < r));
        if let Some((v, exclusive)) = self.min.as_ref().filter(|(v, _)| inside(v)) {
            (bounds.min, bounds.exclusive_min) = if *exclusive { (None, Some(v.clone())) } else { (Some(v.clone()), None) };
        }
        if let Some((v, exclusive)) = self.max.as_ref().filter(|(v, _)| inside(v)) {
            (bounds.max, bounds.exclusive_max) = if *exclusive { (None, Some(v.clone())) } else { (Some(v.clone()), None) };
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
