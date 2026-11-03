//! Postgres type knowledge: the well-known OIDs the rules read, and the registry of the types a
//! statement mentions, read from `pg_type` through the host.

use crate::host::{as_bool, as_strings, as_u32};
use reylo_engine_core::types::{
    ArrayType, Bounds, BytesType, DateTimeType, DateType, DecimalType, DurationType, Field, FloatType, GeometryModel, GeometryType, InstantType, IntegerType, Kind, MapType, Part, Per, Position, RangeType, Size, TextType, TimePart, TimeType, Type, XmlType, Zone, MICROSECOND,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub mod oid {
    pub const BOOL: u32 = 16;
    pub const BYTEA: u32 = 17;
    pub const NAME: u32 = 19;
    pub const INT8: u32 = 20;
    pub const INT2: u32 = 21;
    pub const INT4: u32 = 23;
    pub const TEXT: u32 = 25;
    pub const JSON: u32 = 114;
    pub const UUID: u32 = 2950;
    pub const FLOAT4: u32 = 700;
    pub const FLOAT8: u32 = 701;
    pub const BPCHAR: u32 = 1042;
    pub const VARCHAR: u32 = 1043;
    pub const DATE: u32 = 1082;
    pub const TIME: u32 = 1083;
    pub const TIMESTAMP: u32 = 1114;
    pub const TIMESTAMPTZ: u32 = 1184;
    pub const INTERVAL: u32 = 1186;
    pub const NUMERIC: u32 = 1700;
    pub const UNKNOWN: u32 = 705;
    pub const JSONB: u32 = 3802;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Base,
    Enum,
    Domain,
    Array,
    Composite,
    Range,
    Multirange,
    Other,
}

#[derive(Debug, Clone)]
pub struct PgType {
    pub oid: u32,
    pub name: String,
    pub kind: TypeKind,
    pub elem: Option<u32>,
    pub base: Option<u32>,
    pub labels: Vec<String>,
    /// A composite type's own columns, in attribute order: the shape a whole-row value carries.
    /// A `record` has none, because an anonymous row type has no catalog entry to read them from.
    pub fields: Vec<Attribute>,
    /// The name a cast may spell, schema-qualified when the search path would not find it.
    pub cast_name: String,
    /// A composite type's relation, whose columns' CHECKs [`TypeRegistry::documents`] keys by it.
    pub relid: Option<u32>,
    /// `typdelim` where it is not a comma: what separates the elements of an array of the type.
    pub delimiter: Option<String>,
    /// The schema the type is defined in; `None` where the host did not say.
    pub namespace: Option<String>,
    /// A range's or a multirange's subtype.
    pub subtype: Option<u32>,
}

impl PgType {
    /// The type's name as the format writes it: bare for a `pg_catalog` type (`int4`), qualified
    /// by its schema for any other (`public.mood`, `extensions.vector`), each part quoted as
    /// Postgres quotes an identifier where it must be.
    pub fn qualified(&self) -> String {
        match self.namespace.as_deref() {
            None | Some("pg_catalog") => self.name.clone(),
            Some(namespace) => format!("{}.{}", quote_ident(namespace), quote_ident(&self.name)),
        }
    }
}

/// An identifier as Postgres writes it: bare where it is lowercase letters, digits and
/// underscores not starting with a digit, else in double quotes.
fn quote_ident(name: &str) -> String {
    let plain = name.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '_') && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if plain { name.to_string() } else { format!("\"{}\"", name.replace('"', "\"\"")) }
}

/// One column of a composite type, read from `pg_attribute` beside its relation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,
    pub type_oid: u32,
    pub not_null: bool,
    /// Its number in the composite's relation; 0 where it was not read.
    #[serde(default)]
    pub attnum: i32,
}

/// The types a statement mentions, by oid. Loaded lazily: the engine asks the host for the oids
/// it meets, then for the element and base types those name, until nothing is missing.
#[derive(Debug, Clone, Default)]
pub struct TypeRegistry {
    pub types: HashMap<u32, PgType>,
    /// By relation and attribute, the canonical schema of what a CHECK's JSON Schema lets into a
    /// JSON column of a composite's relation ([`crate::schema_type`]), which a stored row of it
    /// holds ([`TypeRegistry::render_row`]).
    pub documents: HashMap<(u32, i32), serde_json::Value>,
    /// By relation and attribute, the strings a CHECK keeps a string column of a composite's
    /// relation to, which a stored row of it holds ([`TypeRegistry::render_row`]) as a bare read of
    /// the column does.
    pub values: HashMap<(u32, i32), Vec<String>>,
    /// By relation and attribute, the non-finite values a CHECK keeps out of a float or `numeric`
    /// column of a composite's relation, which a stored row of it holds no more.
    pub non_finite: HashMap<(u32, i32), crate::types::NonFinite>,
}

/// Whether every row a scan of relation `$rel` reads has a value in the column named `$col`, as
/// the planner reads it: the column's `NOT NULL` is valid, and so is the same column's in every
/// inheritance descendant. `pg_attribute.attnotnull` alone says neither. PostgreSQL 18 sets it for
/// a constraint added `NOT VALID`, which leaves the rows already there unchecked ("a (possibly
/// invalid) not-null constraint"), and a parent's flag says nothing of a child that allows NULL
/// (`ALTER TABLE ONLY parent ALTER COLUMN x SET NOT NULL`), whose rows a scan of the parent reads
/// and RowDescription reports as the parent's. Before 18 no not-null row is in `pg_constraint`, so
/// the validity test passes there by itself. A partitioned table's partitions hold its `NOT NULL`,
/// except a foreign one: the server enforces no constraint on a foreign table's rows, so neither
/// a foreign table nor a relation a scan of which reads one holds it.
macro_rules! not_null_rows {
    ($rel:literal, $col:literal) => {
        concat!(
            "NOT EXISTS (WITH RECURSIVE tree(rel) AS (SELECT ", $rel, " UNION SELECT i.inhrelid FROM tree t JOIN pg_catalog.pg_inherits i ON i.inhparent OPERATOR(pg_catalog.=) t.rel) \
    SELECT 1 FROM tree t JOIN pg_catalog.pg_attribute d ON d.attrelid OPERATOR(pg_catalog.=) t.rel AND d.attname OPERATOR(pg_catalog.=) ", $col, " AND NOT d.attisdropped \
    WHERE NOT d.attnotnull OR EXISTS (SELECT 1 FROM pg_catalog.pg_constraint k WHERE k.conrelid OPERATOR(pg_catalog.=) d.attrelid \
      AND k.contype OPERATOR(pg_catalog.=) 'n'::pg_catalog.\"char\" AND k.conkey[1] OPERATOR(pg_catalog.=) d.attnum AND NOT k.convalidated) \
      OR EXISTS (SELECT 1 FROM pg_catalog.pg_class f WHERE f.oid OPERATOR(pg_catalog.=) t.rel AND f.relkind OPERATOR(pg_catalog.=) 'f'::pg_catalog.\"char\"))"
        )
    };
}
pub(crate) use not_null_rows;

/// One value per column of the composite type `t`, in attribute order, as an array; NULL for a
/// type that is no composite or has no column.
macro_rules! composite_fields {
    ($($value:tt)+) => {
        concat!(
            "CASE WHEN t.typtype OPERATOR(pg_catalog.=) 'c'::pg_catalog.\"char\" AND EXISTS (SELECT 1 FROM pg_catalog.pg_attribute a \
      WHERE a.attrelid OPERATOR(pg_catalog.=) t.typrelid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped) \
      THEN ARRAY(SELECT ", $($value)+, " FROM pg_catalog.pg_attribute a \
      WHERE a.attrelid OPERATOR(pg_catalog.=) t.typrelid AND a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped ORDER BY a.attnum) END"
        )
    };
}

/// The columns of one `pg_type` row `t` as [`TypeRegistry::insert_rows`] reads them: the oid, the
/// name, `typtype`, the element and base types, an enum's labels, the name a cast spells, and a
/// composite's columns from `pg_attribute` as three arrays in attribute order (names, type oids,
/// `NOT NULL`), so a whole-row value has its fields in the same round as its type, the delimiter
/// of an array of it, its schema, and a range's or multirange's subtype. An enum of no label, or a
/// composite of no column, has NULL for its arrays.
macro_rules! type_columns {
    () => {
        concat!("t.oid::pg_catalog.int8, t.typname::pg_catalog.text, t.typtype::pg_catalog.text, t.typelem::pg_catalog.int8, t.typbasetype::pg_catalog.int8, \
    CASE WHEN t.typtype OPERATOR(pg_catalog.=) 'e'::pg_catalog.\"char\" AND EXISTS (SELECT 1 FROM pg_catalog.pg_enum e WHERE e.enumtypid OPERATOR(pg_catalog.=) t.oid) \
      THEN ARRAY(SELECT e.enumlabel::pg_catalog.text FROM pg_catalog.pg_enum e WHERE e.enumtypid OPERATOR(pg_catalog.=) t.oid ORDER BY e.enumsortorder) END, \
    pg_catalog.format_type(t.oid, NULL::pg_catalog.int4), ",
    composite_fields!("a.attname::pg_catalog.text"), ", ",
    composite_fields!("a.atttypid::pg_catalog.int8"), ", ",
    composite_fields!(concat!("a.attnotnull AND ", not_null_rows!("t.typrelid", "a.attname"))), ", \
    CASE WHEN t.typtype OPERATOR(pg_catalog.=) 'c'::pg_catalog.\"char\" THEN t.typrelid::pg_catalog.int8 END, ",
    composite_fields!("a.attnum::pg_catalog.int4"), ", \
    t.typdelim::pg_catalog.text, \
    (SELECT n.nspname::pg_catalog.text FROM pg_catalog.pg_namespace n WHERE n.oid OPERATOR(pg_catalog.=) t.typnamespace), \
    (SELECT r.rngsubtype::pg_catalog.int8 FROM pg_catalog.pg_range r WHERE r.rngtypid OPERATOR(pg_catalog.=) t.oid OR r.rngmultitypid OPERATOR(pg_catalog.=) t.oid)")
    };
}

/// `$1` is `oid[]`.
pub const TYPE_QUERY: &str = concat!("SELECT ", type_columns!(), " FROM pg_catalog.pg_type t WHERE t.oid OPERATOR(pg_catalog.=) ANY ($1::pg_catalog.oid[])");

/// The same rows for every column type of the named relations, with the element and base types
/// those name, so a statement over plain columns has its types before it is described. `$1` is
/// `text[]` of relation names as the statement writes them, `$2` how many.
pub const RELATION_TYPES_QUERY: &str = concat!(
    "WITH cols AS (SELECT a.atttypid AS oid FROM ",
    crate::cardinality::names!("$1::pg_catalog.text[]", "$2::pg_catalog.int4", "x"),
    " JOIN pg_catalog.pg_attribute a ON a.attrelid OPERATOR(pg_catalog.=) ",
    crate::cardinality::relation_named!("x.name"),
    " \
      WHERE a.attnum OPERATOR(pg_catalog.>) 0 AND NOT a.attisdropped), \
    wanted AS (SELECT cols.oid FROM cols \
      UNION SELECT t.typelem FROM pg_catalog.pg_type t JOIN cols ON cols.oid OPERATOR(pg_catalog.=) t.oid WHERE t.typelem OPERATOR(pg_catalog.<>) '0'::pg_catalog.oid \
      UNION SELECT t.typbasetype FROM pg_catalog.pg_type t JOIN cols ON cols.oid OPERATOR(pg_catalog.=) t.oid WHERE t.typbasetype OPERATOR(pg_catalog.<>) '0'::pg_catalog.oid) \
    SELECT ",
    type_columns!(),
    " FROM pg_catalog.pg_type t WHERE t.oid OPERATOR(pg_catalog.=) ANY (SELECT w.oid FROM wanted w)"
);

/// The `pg_catalog` types with fixed oids the registry starts with, as `pg_type` reports them on
/// 14 (`oid, typname, typtype, typelem, typbasetype, format_type`), so a statement over plain
/// columns and built-in expressions needs no type query at all. An oid below 10000 is assigned
/// in `pg_type.dat` and does not change between releases.
const BUILTINS: &[(u32, &str, &str, u32, u32, &str)] = &[
    (16, "bool", "b", 0, 0, "boolean"),
    (17, "bytea", "b", 0, 0, "bytea"),
    (18, "char", "b", 0, 0, "\"char\""),
    (19, "name", "b", 18, 0, "name"),
    (20, "int8", "b", 0, 0, "bigint"),
    (21, "int2", "b", 0, 0, "smallint"),
    (23, "int4", "b", 0, 0, "integer"),
    (25, "text", "b", 0, 0, "text"),
    (26, "oid", "b", 0, 0, "oid"),
    (114, "json", "b", 0, 0, "json"),
    (142, "xml", "b", 0, 0, "xml"),
    (143, "_xml", "b", 142, 0, "xml[]"),
    (199, "_json", "b", 114, 0, "json[]"),
    (650, "cidr", "b", 0, 0, "cidr"),
    (651, "_cidr", "b", 650, 0, "cidr[]"),
    (700, "float4", "b", 0, 0, "real"),
    (701, "float8", "b", 0, 0, "double precision"),
    (705, "unknown", "p", 0, 0, "unknown"),
    (790, "money", "b", 0, 0, "money"),
    (791, "_money", "b", 790, 0, "money[]"),
    (829, "macaddr", "b", 0, 0, "macaddr"),
    (869, "inet", "b", 0, 0, "inet"),
    (1000, "_bool", "b", 16, 0, "boolean[]"),
    (1001, "_bytea", "b", 17, 0, "bytea[]"),
    (1002, "_char", "b", 18, 0, "\"char\"[]"),
    (1003, "_name", "b", 19, 0, "name[]"),
    (1005, "_int2", "b", 21, 0, "smallint[]"),
    (1007, "_int4", "b", 23, 0, "integer[]"),
    (1009, "_text", "b", 25, 0, "text[]"),
    (1014, "_bpchar", "b", 1042, 0, "character[]"),
    (1015, "_varchar", "b", 1043, 0, "character varying[]"),
    (1016, "_int8", "b", 20, 0, "bigint[]"),
    (1021, "_float4", "b", 700, 0, "real[]"),
    (1022, "_float8", "b", 701, 0, "double precision[]"),
    (1028, "_oid", "b", 26, 0, "oid[]"),
    (1040, "_macaddr", "b", 829, 0, "macaddr[]"),
    (1041, "_inet", "b", 869, 0, "inet[]"),
    (1042, "bpchar", "b", 0, 0, "character"),
    (1043, "varchar", "b", 0, 0, "character varying"),
    (1082, "date", "b", 0, 0, "date"),
    (1083, "time", "b", 0, 0, "time without time zone"),
    (1114, "timestamp", "b", 0, 0, "timestamp without time zone"),
    (1115, "_timestamp", "b", 1114, 0, "timestamp without time zone[]"),
    (1182, "_date", "b", 1082, 0, "date[]"),
    (1183, "_time", "b", 1083, 0, "time without time zone[]"),
    (1184, "timestamptz", "b", 0, 0, "timestamp with time zone"),
    (1185, "_timestamptz", "b", 1184, 0, "timestamp with time zone[]"),
    (1186, "interval", "b", 0, 0, "interval"),
    (1187, "_interval", "b", 1186, 0, "interval[]"),
    (1231, "_numeric", "b", 1700, 0, "numeric[]"),
    (1266, "timetz", "b", 0, 0, "time with time zone"),
    (1270, "_timetz", "b", 1266, 0, "time with time zone[]"),
    (1560, "bit", "b", 0, 0, "bit"),
    (1562, "varbit", "b", 0, 0, "bit varying"),
    (1700, "numeric", "b", 0, 0, "numeric"),
    (2205, "regclass", "b", 0, 0, "regclass"),
    (2206, "regtype", "b", 0, 0, "regtype"),
    (2210, "_regclass", "b", 2205, 0, "regclass[]"),
    (2211, "_regtype", "b", 2206, 0, "regtype[]"),
    (2249, "record", "p", 0, 0, "record"),
    (2278, "void", "p", 0, 0, "void"),
    (2950, "uuid", "b", 0, 0, "uuid"),
    (2951, "_uuid", "b", 2950, 0, "uuid[]"),
    (3614, "tsvector", "b", 0, 0, "tsvector"),
    (3615, "tsquery", "b", 0, 0, "tsquery"),
    (3802, "jsonb", "b", 0, 0, "jsonb"),
    (3807, "_jsonb", "b", 3802, 0, "jsonb[]"),
    (3904, "int4range", "r", 0, 0, "int4range"),
    (3905, "_int4range", "b", 3904, 0, "int4range[]"),
    (3906, "numrange", "r", 0, 0, "numrange"),
    (3907, "_numrange", "b", 3906, 0, "numrange[]"),
    (3908, "tsrange", "r", 0, 0, "tsrange"),
    (3909, "_tsrange", "b", 3908, 0, "tsrange[]"),
    (3910, "tstzrange", "r", 0, 0, "tstzrange"),
    (3911, "_tstzrange", "b", 3910, 0, "tstzrange[]"),
    (3912, "daterange", "r", 0, 0, "daterange"),
    (3913, "_daterange", "b", 3912, 0, "daterange[]"),
    (3926, "int8range", "r", 0, 0, "int8range"),
    (3927, "_int8range", "b", 3926, 0, "int8range[]"),
];

/// How deep [`TypeRegistry::render`] and [`TypeRegistry::missing`] follow arrays and composite
/// columns; the catalog forbids a composite that contains itself, so this only guards a registry
/// a host filled by hand.
const MAX_DEPTH: usize = 16;

impl TypeRegistry {
    /// A registry holding [`BUILTINS`].
    pub fn with_builtins() -> TypeRegistry {
        let mut registry = TypeRegistry::default();
        let subtype = |oid: u32| match oid {
            3904 => Value::from(oid::INT4),
            3906 => Value::from(oid::NUMERIC),
            3908 => Value::from(oid::TIMESTAMP),
            3910 => Value::from(oid::TIMESTAMPTZ),
            3912 => Value::from(oid::DATE),
            3926 => Value::from(oid::INT8),
            _ => Value::Null,
        };
        let rows: Vec<Vec<Value>> = BUILTINS
            .iter()
            .map(|(oid, name, typtype, elem, base, cast)| {
                let mut row = vec![Value::from(*oid), Value::from(*name), Value::from(*typtype), Value::from(*elem), Value::from(*base), Value::Null, Value::from(*cast)];
                row.extend([Value::Null, Value::Null, Value::Null, Value::Null, Value::Null, Value::Null, Value::from("pg_catalog"), subtype(*oid)]);
                row
            })
            .collect();
        registry.insert_rows(&rows);
        registry
    }

    /// Rows of [`TYPE_QUERY`] as the host hands them back; a driver may decode `int8` as a
    /// string, so every number is read leniently.
    pub fn insert_rows(&mut self, rows: &[Vec<Value>]) {
        for row in rows {
            let (Some(oid), Some(name), Some(typtype)) = (row.first().and_then(as_u32), row.get(1).and_then(Value::as_str), row.get(2).and_then(Value::as_str)) else {
                continue;
            };
            let elem = row.get(3).and_then(as_u32).unwrap_or(0);
            let base = row.get(4).and_then(as_u32).unwrap_or(0);
            let labels = row.get(5).and_then(as_strings);
            let cast_name = row.get(6).and_then(Value::as_str).unwrap_or("").to_string();
            let attnums: Vec<i32> = row.get(11).and_then(Value::as_array).map(|a| a.iter().map(|n| as_u32(n).unwrap_or(0) as i32).collect()).unwrap_or_default();
            let fields = match (row.get(7).and_then(as_strings), row.get(8).and_then(Value::as_array), row.get(9).and_then(Value::as_array)) {
                (Some(names), Some(oids), Some(not_null)) if names.len() == oids.len() && names.len() == not_null.len() => names
                    .into_iter()
                    .zip(oids)
                    .zip(not_null)
                    .enumerate()
                    .filter_map(|(i, ((name, o), n))| Some(Attribute { name, type_oid: as_u32(o)?, not_null: as_bool(n)?, attnum: attnums.get(i).copied().unwrap_or(0) }))
                    .collect(),
                _ => Vec::new(),
            };
            let relid = row.get(10).and_then(as_u32).filter(|r| *r != 0);
            let delimiter = row.get(12).and_then(Value::as_str).filter(|d| *d != ",").map(str::to_string);
            let namespace = row.get(13).and_then(Value::as_str).map(str::to_string);
            let subtype = row.get(14).and_then(as_u32).filter(|s| *s != 0);
            let name = name.to_string();
            let kind = match typtype {
                "b" | "p" if elem != 0 && name.starts_with('_') => TypeKind::Array,
                "b" => TypeKind::Base,
                "e" => TypeKind::Enum,
                "d" => TypeKind::Domain,
                "c" => TypeKind::Composite,
                "r" => TypeKind::Range,
                "m" => TypeKind::Multirange,
                _ => TypeKind::Other,
            };
            self.types.insert(
                oid,
                PgType { oid, name, kind, elem: (elem != 0).then_some(elem), base: (base != 0).then_some(base), labels: labels.unwrap_or_default(), fields, cast_name, relid, delimiter, namespace, subtype },
            );
        }
    }

    /// Mark an oid the catalog did not answer for, so it is not asked for again.
    pub fn insert_unknown(&mut self, oid: u32) {
        self.types.entry(oid).or_insert(PgType {
            oid,
            name: format!("oid:{oid}"),
            kind: TypeKind::Other,
            elem: None,
            base: None,
            labels: Vec::new(),
            fields: Vec::new(),
            cast_name: String::new(),
            relid: None,
            delimiter: None,
            namespace: None,
            subtype: None,
        });
    }

    pub fn get(&self, oid: u32) -> Option<&PgType> {
        self.types.get(&oid)
    }

    pub fn name(&self, oid: u32) -> String {
        self.get(oid).map(|t| t.name.clone()).unwrap_or_else(|| format!("oid:{oid}"))
    }

    /// Resolve a domain to its base type oid (transitively).
    pub fn resolve_domain(&self, oid: u32) -> u32 {
        let mut cur = oid;
        let mut hops = 0;
        while let Some(t) = self.get(cur) {
            match (t.kind, t.base) {
                (TypeKind::Domain, Some(b)) if hops < 8 => {
                    cur = b;
                    hops += 1;
                }
                _ => break,
            }
        }
        cur
    }

    /// The oids this one needs to be rendered, itself included, that the registry does not hold:
    /// through arrays, domains and a composite's columns.
    pub fn missing(&self, oid: u32, out: &mut Vec<u32>) {
        self.missing_at(oid, out, 0);
    }

    fn missing_at(&self, oid: u32, out: &mut Vec<u32>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let Some(t) = self.get(oid) else {
            if !out.contains(&oid) {
                out.push(oid);
            }
            return;
        };
        match (t.kind, t.elem, t.base) {
            (TypeKind::Array, Some(e), _) => self.missing_at(e, out, depth + 1),
            (TypeKind::Domain, _, Some(b)) => self.missing_at(b, out, depth + 1),
            (TypeKind::Range | TypeKind::Multirange, _, _) if t.subtype.is_some() => self.missing_at(t.subtype.unwrap_or(0), out, depth + 1),
            (TypeKind::Composite, _, _) => {
                for f in &t.fields {
                    self.missing_at(f.type_oid, out, depth + 1);
                }
            }
            _ => {}
        }
    }

    /// The JSON columns of the relation whose row type `oid` is, by relation and attribute: the
    /// ones whose CHECKs a stored row of it was held to.
    pub fn row_json_columns(&self, oid: u32) -> Vec<(u32, i32)> {
        self.row_columns(oid, &[oid::JSON, oid::JSONB])
    }

    /// The string columns of the relation whose row type `oid` is, by relation and attribute: the
    /// ones a CHECK may keep to a list of strings ([`TypeRegistry::values`]).
    pub fn row_string_columns(&self, oid: u32) -> Vec<(u32, i32)> {
        self.row_columns(oid, &[oid::TEXT, oid::VARCHAR])
    }

    /// The float and `numeric` columns of the relation whose row type `oid` is, by relation and
    /// attribute: the ones a CHECK may keep NaN or the infinities out of ([`TypeRegistry::non_finite`]).
    pub fn row_number_columns(&self, oid: u32) -> Vec<(u32, i32)> {
        self.row_columns(oid, &[oid::FLOAT4, oid::FLOAT8, oid::NUMERIC])
    }

    fn row_columns(&self, oid: u32, types: &[u32]) -> Vec<(u32, i32)> {
        // An array of stored rows holds rows of its element's relation.
        let oid = self.get(oid).filter(|t| t.kind == TypeKind::Array).and_then(|t| t.elem).unwrap_or(oid);
        let Some(t) = self.get(oid).filter(|t| t.kind == TypeKind::Composite) else { return Vec::new() };
        let Some(relid) = t.relid else { return Vec::new() };
        t.fields.iter().filter(|f| f.attnum > 0 && types.contains(&self.resolve_domain(f.type_oid))).map(|f| (relid, f.attnum)).collect()
    }

    /// The type of a stored row of the relation whose row type `oid` is (`d` of `from docs d`):
    /// the table's constraints held it, so a column is non-null where its `NOT NULL` is valid, a
    /// JSON column a CHECK holds to a JSON Schema is the values it admits
    /// ([`TypeRegistry::documents`]), a string column a CHECK keeps to a list of strings is those
    /// strings ([`TypeRegistry::values`]), and a float or `numeric` column holds no non-finite
    /// value a CHECK keeps out ([`TypeRegistry::non_finite`]). A row value of the same type that a statement
    /// builds (`row(…)::docs`), or that another table's column holds, met none of them, and
    /// renders as [`TypeRegistry::render`] does.
    pub fn render_row(&self, oid: u32) -> Type {
        self.render_at(oid, 0, true)
    }

    /// The type of one oid. A composite's fields are each nullable: a row value of a table's type
    /// need not be one of its stored rows ([`TypeRegistry::render_row`]).
    pub fn render(&self, oid: u32) -> Type {
        self.render_at(oid, 0, false)
    }

    /// What separates the elements of an array of type `oid`'s text, where it is not a comma: its
    /// element type's `typdelim` (`:` for PostGIS's `geometry`), a domain's its base type's.
    pub fn delimiter(&self, oid: u32) -> Option<String> {
        let t = self.get(oid).filter(|t| t.kind == TypeKind::Array)?;
        self.get(t.elem?).and_then(|e| e.delimiter.clone())
    }

    /// Whether type `oid` is `text` or `varchar`, through any domain: a string a CHECK may keep to
    /// a list of strings ([`with_values`]).
    pub fn is_string(&self, oid: u32) -> bool {
        matches!(self.resolve_domain(oid), oid::TEXT | oid::VARCHAR)
    }

    /// Whether type `oid` is a float or `numeric`, through any domain: a number a CHECK may keep
    /// NaN or the infinities out of ([`crate::types::NonFinite`]).
    pub fn is_number(&self, oid: u32) -> bool {
        matches!(self.resolve_domain(oid), oid::FLOAT4 | oid::FLOAT8 | oid::NUMERIC)
    }

    /// Each domain among `names` with the name of the base type it is made from, through any
    /// domain between: a domain's values are sent and decoded as its base type's.
    pub fn domain_bases(&self, names: &[String]) -> std::collections::BTreeMap<String, String> {
        self.types
            .values()
            .filter(|t| t.kind == TypeKind::Domain && names.contains(&t.qualified()))
            .filter_map(|t| {
                let base = self.get(self.resolve_domain(t.oid)).filter(|b| b.kind != TypeKind::Domain)?;
                Some((t.qualified(), base.qualified()))
            })
            .collect()
    }

    fn render_at(&self, oid: u32, depth: usize, stored: bool) -> Type {
        if depth > MAX_DEPTH {
            return Type::unknown();
        }
        let Some(t) = self.get(oid) else { return Type::unknown() };
        match t.kind {
            // A Postgres array holds any number of dimensions, whatever a column declares, and an
            // element may be NULL whatever a column's NOT NULL says. The elements of an array of
            // stored rows are stored rows.
            TypeKind::Array => match t.elem {
                Some(e) => Type::new(Kind::Array(ArrayType { element: Box::new(Position::nullable(self.render_at(e, depth + 1, stored))), size: None, any_depth: true })).named(t.qualified()),
                None => Type::unknown(),
            },
            // A domain is its base type's values under its own name.
            TypeKind::Domain => match t.base.map(|b| self.render_at(b, depth + 1, stored)) {
                Some(base) if !base.is_unknown() => base.named(t.qualified()),
                _ => Type::unknown(),
            },
            TypeKind::Enum => Type::text_of(t.labels.clone()).named(t.qualified()),
            TypeKind::Composite if !t.fields.is_empty() => {
                let fields = t
                    .fields
                    .iter()
                    .map(|f| {
                        let ty = self.render_at(f.type_oid, depth + 1, false);
                        let document = t.relid.filter(|_| stored).and_then(|r| self.documents.get(&(r, f.attnum)));
                        let values = t.relid.filter(|_| stored).and_then(|r| self.values.get(&(r, f.attnum)));
                        let non_finite = t.relid.filter(|_| stored).and_then(|r| self.non_finite.get(&(r, f.attnum)));
                        let string = self.is_string(f.type_oid);
                        let ty = match (document, values, non_finite) {
                            (Some(schema), _, _) if ty.is_json() && ty.schema().is_none() => Type { kind: Type::json_schema_of(schema.clone()).kind, ..ty },
                            (_, Some(values), _) if string => with_values(ty, values),
                            (_, _, Some(kept_out)) => kept_out.apply(ty),
                            _ => ty,
                        };
                        Field::new(f.name.clone(), ty, !(stored && f.not_null))
                    })
                    .collect();
                Type::record(fields).named(t.qualified())
            }
            // A row value whose columns were not read: no driver parses one, so it reaches the
            // caller as its text form, and so does an array of them.
            TypeKind::Composite => Type::opaque("record"),
            TypeKind::Range | TypeKind::Multirange => match t.subtype.map(|s| self.render_at(s, depth + 1, false)) {
                Some(subtype) if !subtype.is_unknown() => {
                    let range = RangeType { subtype: Box::new(subtype) };
                    Type::new(if t.kind == TypeKind::Range { Kind::Range(range) } else { Kind::Multirange(range) }).named(t.qualified())
                }
                _ => Type::opaque(t.qualified()),
            },
            TypeKind::Base | TypeKind::Other => base_type(t),
        }
    }
}

/// A `text` or `varchar` value kept to a CHECK's list of strings (`status IN ('active',
/// 'banned')`): those strings alone. The caller says the type is one of the two, through any
/// domain; `bpchar` pads what it stores, so its values are not the list's.
pub fn with_values(ty: Type, values: &[String]) -> Type {
    match ty.kind {
        Kind::Text(text) if !values.is_empty() => Type { kind: Kind::Text(TextType { values: Some(values.to_vec()), ..text }), ..ty },
        kind => Type { kind, ..ty },
    }
}

/// Postgres's floats, which hold infinity, -infinity and NaN.
fn pg_float(exponent_bits: u32, mantissa_bits: u32, name: &str) -> Type {
    Type::new(Kind::Float(FloatType { exponent_bits, mantissa_bits, infinity: true, nan: true, ..FloatType::default() })).named(name)
}

/// The earliest and latest dates, as Postgres stores them.
const DATE_MIN: &str = "-4713-11-24";
const DATE_MAX: &str = "+5874897-12-31";

/// A base type, or a type the catalog gives no structure of its own (an extension's, a
/// pseudo-type), by its name: the kind and fields of what every value is. A type the table does
/// not name is opaque, which says nothing of what a value is but its name.
fn base_type(t: &PgType) -> Type {
    let name = t.qualified();
    let point = || Type::record(vec![Field::new("x", pg_float(11, 52, "float8"), false), Field::new("y", pg_float(11, 52, "float8"), false)]).named("point");
    let points = |min: u64| Type::new(Kind::Array(ArrayType { element: Box::new(Position::non_null(point())), size: Some(Size { min: Some(min), ..Size::default() }), any_depth: false }));
    let float8 = || pg_float(11, 52, "float8");
    let bits = || Type::array(Position::non_null(Type::boolean()));
    let vector = |exponent_bits, mantissa_bits| Type::array(Position::non_null(Type::float(exponent_bits, mantissa_bits)));
    let address = |bytes: u64, prefix: u32| {
        Type::record(vec![
            Field::new("address", Type::new(Kind::Bytes(BytesType { size: Some(Size::exact(bytes)) })), false),
            Field::new("prefix", Type::new(Kind::Integer(IntegerType { bits: 8, unsigned: true, bounds: Bounds { max: Some(prefix.to_string()), ..Bounds::default() }, ..IntegerType::default() })), false),
        ])
    };
    let builtin = t.namespace.as_deref().is_none_or(|n| n == "pg_catalog");
    let ty = match (builtin, t.name.as_str()) {
        (true, "bool") => Type::boolean(),
        (true, "int2") => Type::integer(16),
        (true, "int4") => Type::integer(32),
        (true, "int8") => Type::integer(64),
        (true, "oid") => Type::unsigned(32),
        (true, "float4") => return pg_float(8, 23, "float4"),
        (true, "float8") => return float8(),
        // With no type modifier: a scale of 0 to 16,383 and below 10^131072 in magnitude.
        (true, "numeric") => Type::decimal(DecimalType {
            min_scale: Some(0),
            max_scale: Some(16383),
            bounds: Bounds { exclusive_min: Some("-1e131072".into()), exclusive_max: Some("1e131072".into()), ..Bounds::default() },
            infinity: true,
            nan: true,
            ..DecimalType::default()
        }),
        (true, "text" | "varchar" | "bpchar" | "name" | "char" | "unknown") => Type::text(),
        (true, "bytea") => Type::bytes(),
        (true, "uuid") => Type::uuid(),
        (true, "date") => Type::new(Kind::Date(DateType { bounds: Bounds::between(DATE_MIN, DATE_MAX), infinity: true })),
        (true, "timestamp") => Type::new(Kind::DateTime(DateTimeType { tick: MICROSECOND.into(), bounds: Bounds::between("-4713-11-24T00:00:00", "+294276-12-31T23:59:59.999999"), infinity: true })),
        (true, "timestamptz") => Type::new(Kind::Instant(InstantType { tick: MICROSECOND.into(), zone: None, bounds: Bounds::between("-4713-11-24T00:00:00Z", "+294276-12-31T23:59:59.999999Z"), infinity: true })),
        (true, "time") => Type::new(Kind::Time(TimeType { tick: MICROSECOND.into(), zone: None, bounds: Bounds { max: Some("24:00:00".into()), ..Bounds::default() } })),
        (true, "timetz") => Type::new(Kind::Time(TimeType { tick: MICROSECOND.into(), zone: Some(Zone::Offset), bounds: Bounds { max: Some("24:00:00".into()), ..Bounds::default() } })),
        // Infinite intervals came with version 17, which the engine does not tell apart: an
        // interval may be infinite.
        (true, "interval") => Type::new(Kind::Duration(DurationType {
            months: Some(Part { max: Some("P2147483648M".into()) }),
            days: Some(Part { max: Some("P2147483648D".into()) }),
            time: Some(TimePart { tick: MICROSECOND.into(), max: Some("PT2562047788H54.775808S".into()) }),
            infinity: true,
        })),
        (true, "json" | "jsonb") => Type::json(),
        (true, "xml") => Type::new(Kind::Xml(XmlType { fragment: true, schemas: None })),
        (true, "bit" | "varbit") => bits(),
        (true, "inet" | "cidr") => Type::union(vec![address(4, 32), address(16, 128)], Per::Value),
        (true, "macaddr") => Type::new(Kind::Bytes(BytesType { size: Some(Size::exact(6)) })),
        (true, "macaddr8") => Type::new(Kind::Bytes(BytesType { size: Some(Size::exact(8)) })),
        (true, "point") => return point(),
        (true, "line") => Type::record(vec![Field::new("a", float8(), false), Field::new("b", float8(), false), Field::new("c", float8(), false)]),
        (true, "lseg") => Type::new(Kind::Array(ArrayType { element: Box::new(Position::non_null(point())), size: Some(Size::exact(2)), any_depth: false })),
        (true, "box") => Type::record(vec![Field::new("high", point(), false), Field::new("low", point(), false)]),
        (true, "path") => Type::record(vec![Field::new("closed", Type::boolean(), false), Field::new("points", points(1), false)]),
        (true, "polygon") => points(1),
        (true, "circle") => Type::record(vec![Field::new("center", point(), false), Field::new("radius", float8(), false)]),
        // An extension's types, in whatever schema it was created in.
        (_, "hstore") => Type::new(Kind::Map(MapType { key: Box::new(Type::text()), value: Box::new(Position::nullable(Type::text())) })),
        (_, "vector" | "sparsevec") => vector(8, 23),
        (_, "halfvec") => vector(5, 10),
        (_, "citext") => Type::text(),
        (_, "geometry") => Type::new(Kind::Geometry(GeometryType { model: Some(GeometryModel::Planar), ..GeometryType::default() })),
        (_, "geography") => Type::new(Kind::Geometry(GeometryType { model: Some(GeometryModel::Geodetic), ..GeometryType::default() })),
        _ => return Type::opaque(name),
    };
    ty.named(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A `TYPE_QUERY` row for a composite, its columns as a driver hands them back: one decodes
    /// `int8[]` as numbers, another as digit strings.
    fn users_row(oids: Value) -> Vec<Value> {
        vec![json!(90001), json!("users"), json!("c"), json!(0), json!(0), Value::Null, json!("users"), json!(["id", "name", "tags"]), oids, json!([true, false, false])]
    }

    /// The fifteen columns of TYPE_QUERY for a type with no composite columns.
    fn row(oid: u32, name: &str, typtype: &str, elem: u32, base: u32, delimiter: &str, namespace: &str, subtype: u32) -> Vec<Value> {
        vec![json!(oid), json!(name), json!(typtype), json!(elem), json!(base), Value::Null, json!(name), Value::Null, Value::Null, Value::Null, Value::Null, Value::Null, json!(delimiter), json!(namespace), json!(subtype)]
    }

    #[test]
    fn a_composite_renders_its_columns_once_they_are_read() {
        for oids in [json!([20, 25, 1009]), json!(["20", "25", "1009"])] {
            let mut registry = TypeRegistry::with_builtins();
            registry.insert_rows(&[users_row(oids)]);
            assert_eq!(
                registry.get(90001).unwrap().fields,
                vec![
                    Attribute { name: "id".into(), type_oid: 20, not_null: true, attnum: 0 },
                    Attribute { name: "name".into(), type_oid: 25, not_null: false, attnum: 0 },
                    Attribute { name: "tags".into(), type_oid: 1009, not_null: false, attnum: 0 },
                ]
            );
            // A stored row of the table holds its NOT NULL; a row value of its type need not. A
            // host that does not say the schema leaves the name bare.
            assert_eq!(registry.render_row(90001).short(), "users{ id: int8, name: text?, tags: text?[]? }");
            assert_eq!(registry.render(90001).short(), "users{ id: int8?, name: text?, tags: text?[]? }");
        }
    }

    #[test]
    fn a_composite_without_its_columns_stays_a_record() {
        let mut registry = TypeRegistry::with_builtins();
        // The seven columns a host that predates the composite columns answers with.
        registry.insert_rows(&[vec![json!(90001), json!("users"), json!("c"), json!(0), json!(0), Value::Null, json!("users")]]);
        assert!(registry.get(90001).unwrap().fields.is_empty());
        assert_eq!(registry.render(90001), Type::opaque("record"));
        // `record` itself is a pseudo-type with no columns.
        assert_eq!(registry.render(2249), Type::opaque("record"));
    }

    #[test]
    fn missing_follows_a_composites_columns() {
        let mut registry = TypeRegistry::with_builtins();
        // `id` is a domain the registry has not seen, `tags` an array of an enum it has not seen.
        let users = vec![json!(90001), json!("users"), json!("c"), json!(0), json!(0), Value::Null, json!("users"), json!(["id", "tags"]), json!([90002, 90003]), json!([true, false])];
        registry.insert_rows(&[users]);
        let mut out = Vec::new();
        registry.missing(90001, &mut out);
        assert_eq!(out, vec![90002, 90003]);
        registry.insert_rows(&[row(90002, "user_id", "d", 0, 20, ",", "public", 0), row(90003, "_mood", "b", 90004, 0, ",", "public", 0)]);
        let mut out = Vec::new();
        registry.missing(90001, &mut out);
        assert_eq!(out, vec![90004]);
        let mut mood = row(90004, "mood", "e", 0, 0, ",", "public", 0);
        mood[5] = json!(["ok", "sad"]);
        registry.insert_rows(&[mood]);
        let mut out = Vec::new();
        registry.missing(90001, &mut out);
        assert!(out.is_empty());
        // A domain is its base type's values under its own name, and an enum is its labels.
        assert_eq!(registry.render(90001).short(), "users{ id: public.user_id?, tags: (public.mood('ok' | 'sad')?)[]? }");
        assert_eq!(registry.render(90002), Type::integer(64).named("public.user_id"));
        assert_eq!(registry.domain_bases(&["public.user_id".to_string(), "public.mood".to_string()]), [("public.user_id".to_string(), "int8".to_string())].into_iter().collect());
    }

    #[test]
    fn an_array_carries_its_elements_delimiter_where_it_is_not_a_comma() {
        let mut registry = TypeRegistry::with_builtins();
        // PostGIS's geometry, its array, a domain over it and the domain's array. A domain has
        // its base type's delimiter.
        registry.insert_rows(&[
            row(90001, "geometry", "b", 0, 0, ":", "public", 0),
            row(90002, "_geometry", "b", 90001, 0, ",", "public", 0),
            row(90003, "place", "d", 0, 90001, ":", "public", 0),
            row(90004, "_place", "b", 90003, 0, ",", "public", 0),
        ]);
        assert_eq!(registry.delimiter(90002).as_deref(), Some(":"));
        assert_eq!(registry.delimiter(90004).as_deref(), Some(":"));
        assert_eq!(registry.render(90002).short(), "public.geometry?[]");
        assert_eq!(registry.render(90004).short(), "public.place?[]");
        // A comma, and a host that predates the column, leave it out.
        assert_eq!(registry.delimiter(1009), None);
    }

    #[test]
    fn each_base_type_is_its_values() {
        let registry = TypeRegistry::with_builtins();
        let json = |oid: u32| serde_json::to_value(registry.render(oid)).unwrap();
        assert_eq!(json(oid::INT4), json!({"kind": "integer", "bits": 32, "name": "int4"}));
        assert_eq!(json(26), json!({"kind": "integer", "bits": 32, "unsigned": true, "name": "oid"}));
        assert_eq!(json(oid::FLOAT8), json!({"kind": "float", "exponent_bits": 11, "mantissa_bits": 52, "infinity": true, "nan": true, "name": "float8"}));
        assert_eq!(json(oid::NUMERIC), json!({"kind": "decimal", "min_scale": 0, "max_scale": 16383, "exclusive_min": "-1e131072", "exclusive_max": "1e131072", "infinity": true, "nan": true, "name": "numeric"}));
        assert_eq!(json(oid::DATE), json!({"kind": "date", "min": "-4713-11-24", "max": "+5874897-12-31", "infinity": true, "name": "date"}));
        assert_eq!(json(oid::TIMESTAMPTZ), json!({"kind": "instant", "tick": "1/1000000", "min": "-4713-11-24T00:00:00Z", "max": "+294276-12-31T23:59:59.999999Z", "infinity": true, "name": "timestamptz"}));
        assert_eq!(json(oid::TIME), json!({"kind": "time", "tick": "1/1000000", "max": "24:00:00", "name": "time"}));
        assert_eq!(registry.render(1560).short(), "bit(boolean[])");
        assert_eq!(registry.render(869).short(), "inet({ address: bytes(=4), prefix: uint8(..32) } | { address: bytes(=16), prefix: uint8(..128) })");
        assert_eq!(registry.render(829).short(), "macaddr(=6)");
        assert_eq!(registry.render(3910), Type::new(Kind::Range(RangeType { subtype: Box::new(registry.render(oid::TIMESTAMPTZ)) })).named("tstzrange"));
        assert_eq!(registry.render(3615), Type::opaque("tsquery"));
        assert_eq!(registry.render(oid::JSONB), Type::json().named("jsonb"));
        for oid in registry.types.keys() {
            registry.render(*oid).check().unwrap_or_else(|e| panic!("{oid}: {e}"));
        }
    }

    #[test]
    fn an_extensions_types_are_named_with_their_schema() {
        let mut registry = TypeRegistry::with_builtins();
        registry.insert_rows(&[row(90001, "vector", "b", 0, 0, ",", "extensions", 0), row(90002, "hstore", "b", 0, 0, ",", "public", 0), row(90003, "ltree", "b", 0, 0, ",", "public", 0), row(90004, "Odd Range", "r", 0, 0, ",", "My Schema", 23)]);
        assert_eq!(registry.render(90001).short(), "extensions.vector(float(8,23)[])");
        assert_eq!(serde_json::to_value(registry.render(90001)).unwrap(), json!({"kind": "array", "element": {"type": {"kind": "float", "exponent_bits": 8, "mantissa_bits": 23}}, "name": "extensions.vector"}));
        assert_eq!(registry.render(90002).short(), "public.hstore(text => text?)");
        assert_eq!(registry.render(90003), Type::opaque("public.ltree"));
        assert_eq!(registry.render(90004).name.as_deref(), Some("\"My Schema\".\"Odd Range\""));
    }

    #[test]
    fn a_registry_that_loops_renders_nothing_rather_than_forever() {
        let mut registry = TypeRegistry::default();
        let row = vec![json!(90001), json!("knot"), json!("c"), json!(0), json!(0), Value::Null, json!("knot"), json!(["self"]), json!([90001]), json!([false])];
        registry.insert_rows(&[row]);
        let mut out = Vec::new();
        registry.missing(90001, &mut out);
        assert!(out.is_empty());
        let _ = registry.render(90001);
    }
}
