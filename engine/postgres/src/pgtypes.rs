//! Postgres type knowledge: the well-known OIDs the rules read, and the registry of the types a
//! statement mentions, read from `pg_type` through the host.

use crate::host::{as_bool, as_strings, as_u32};
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
            "NOT EXISTS (WITH RECURSIVE tree(rel) AS (SELECT ", $rel, " UNION SELECT i.inhrelid FROM tree t JOIN pg_inherits i ON i.inhparent = t.rel) \
    SELECT 1 FROM tree t JOIN pg_attribute d ON d.attrelid = t.rel AND d.attname = ", $col, " AND NOT d.attisdropped \
    WHERE NOT d.attnotnull OR EXISTS (SELECT 1 FROM pg_constraint k WHERE k.conrelid = d.attrelid AND k.contype = 'n' AND k.conkey[1] = d.attnum AND NOT k.convalidated) \
      OR EXISTS (SELECT 1 FROM pg_class f WHERE f.oid = t.rel AND f.relkind = 'f'))"
        )
    };
}
pub(crate) use not_null_rows;

/// The columns of one `pg_type` row `t` as [`TypeRegistry::insert_rows`] reads them: the oid, the
/// name, `typtype`, the element and base types, an enum's labels, the name a cast spells, and a
/// composite's columns from `pg_attribute` as three arrays in attribute order (names, type oids,
/// `NOT NULL`), so a whole-row value has its fields in the same round as its type, and the
/// delimiter of an array of it.
macro_rules! type_columns {
    () => {
        concat!("t.oid::int8, t.typname::text, t.typtype::text, t.typelem::int8, t.typbasetype::int8, \
    CASE WHEN t.typtype = 'e' THEN (SELECT array_agg(e.enumlabel::text ORDER BY e.enumsortorder) FROM pg_enum e WHERE e.enumtypid = t.oid) END, \
    pg_catalog.format_type(t.oid, NULL), \
    CASE WHEN t.typtype = 'c' THEN (SELECT array_agg(a.attname::text ORDER BY a.attnum) FROM pg_attribute a WHERE a.attrelid = t.typrelid AND a.attnum > 0 AND NOT a.attisdropped) END, \
    CASE WHEN t.typtype = 'c' THEN (SELECT array_agg(a.atttypid::int8 ORDER BY a.attnum) FROM pg_attribute a WHERE a.attrelid = t.typrelid AND a.attnum > 0 AND NOT a.attisdropped) END, \
    CASE WHEN t.typtype = 'c' THEN (SELECT array_agg(a.attnotnull AND ", not_null_rows!("t.typrelid", "a.attname"), " ORDER BY a.attnum) FROM pg_attribute a WHERE a.attrelid = t.typrelid AND a.attnum > 0 AND NOT a.attisdropped) END, \
    CASE WHEN t.typtype = 'c' THEN t.typrelid::int8 END, \
    CASE WHEN t.typtype = 'c' THEN (SELECT array_agg(a.attnum::int4 ORDER BY a.attnum) FROM pg_attribute a WHERE a.attrelid = t.typrelid AND a.attnum > 0 AND NOT a.attisdropped) END, \
    t.typdelim::text")
    };
}

/// `$1` is `oid[]`.
pub const TYPE_QUERY: &str = concat!("SELECT ", type_columns!(), " FROM pg_type t WHERE t.oid = ANY($1::oid[])");

/// The same rows for every column type of the named relations, with the element and base types
/// those name, so a statement over plain columns has its types before it is described. `$1` is
/// `text[]` of relation names.
pub const RELATION_TYPES_QUERY: &str = concat!(
    "WITH cols AS (SELECT a.atttypid AS oid FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid \
      WHERE c.relname = ANY($1::text[]) AND pg_catalog.pg_table_is_visible(c.oid) AND a.attnum > 0 AND NOT a.attisdropped), \
    wanted AS (SELECT oid FROM cols \
      UNION SELECT t.typelem FROM pg_type t JOIN cols ON cols.oid = t.oid WHERE t.typelem <> 0 \
      UNION SELECT t.typbasetype FROM pg_type t JOIN cols ON cols.oid = t.oid WHERE t.typbasetype <> 0) \
    SELECT ",
    type_columns!(),
    " FROM pg_type t WHERE t.oid IN (SELECT oid FROM wanted)"
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
        let rows: Vec<Vec<Value>> = BUILTINS
            .iter()
            .map(|(oid, name, typtype, elem, base, cast)| vec![Value::from(*oid), Value::from(*name), Value::from(*typtype), Value::from(*elem), Value::from(*base), Value::Null, Value::from(*cast)])
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
            let name = name.to_string();
            let kind = match typtype {
                "b" | "p" if elem != 0 && name.starts_with('_') => TypeKind::Array,
                "b" => TypeKind::Base,
                "e" => TypeKind::Enum,
                "d" => TypeKind::Domain,
                "c" => TypeKind::Composite,
                _ => TypeKind::Other,
            };
            self.types.insert(
                oid,
                PgType { oid, name, kind, elem: (elem != 0).then_some(elem), base: (base != 0).then_some(base), labels: labels.unwrap_or_default(), fields, cast_name, relid, delimiter },
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

    fn row_columns(&self, oid: u32, types: &[u32]) -> Vec<(u32, i32)> {
        // An array of stored rows holds rows of its element's relation.
        let oid = self.get(oid).filter(|t| t.kind == TypeKind::Array).and_then(|t| t.elem).unwrap_or(oid);
        let Some(t) = self.get(oid).filter(|t| t.kind == TypeKind::Composite) else { return Vec::new() };
        let Some(relid) = t.relid else { return Vec::new() };
        t.fields.iter().filter(|f| f.attnum > 0 && types.contains(&self.resolve_domain(f.type_oid))).map(|f| (relid, f.attnum)).collect()
    }

    /// The type model of a stored row of the relation whose row type `oid` is (`d` of `from docs
    /// d`): the table's constraints held it, so a column is non-null where its `NOT NULL` is valid,
    /// a JSON column a CHECK holds to a JSON Schema is the values it admits
    /// ([`TypeRegistry::documents`]), and a string column a CHECK keeps to a list of strings is
    /// their union ([`TypeRegistry::values`]). A row value of the same type that a statement builds
    /// (`row(…)::docs`), or that another table's column holds, met none of them, and renders as
    /// [`TypeRegistry::render`] does.
    pub fn render_row(&self, oid: u32) -> crate::types::Type {
        self.render_at(oid, 0, true)
    }

    /// The type model for one oid. A composite's fields are each nullable: a row value of a table's
    /// type need not be one of its stored rows ([`TypeRegistry::render_row`]).
    pub fn render(&self, oid: u32) -> crate::types::Type {
        self.render_at(oid, 0, false)
    }

    fn render_at(&self, oid: u32, depth: usize, stored: bool) -> crate::types::Type {
        use crate::types::{Field, Type};
        if depth > MAX_DEPTH {
            return Type::Unknown;
        }
        let mut cur = oid;
        for _ in 0..8 {
            let Some(t) = self.get(cur) else { return Type::Unknown };
            match (t.kind, t.elem, t.base) {
                // The elements of an array of stored rows are stored rows. A domain has its base
                // type's delimiter.
                (TypeKind::Array, Some(e), _) => {
                    let delimiter = self.get(e).and_then(|t| t.delimiter.clone());
                    return Type::Array { of: Box::new(self.render_at(e, depth + 1, stored)), nullable_items: None, delimiter };
                }
                (TypeKind::Domain, _, Some(b)) => cur = b,
                (TypeKind::Enum, _, _) => return Type::Enum { name: t.name.clone(), labels: t.labels.clone() },
                (TypeKind::Composite, _, _) if !t.fields.is_empty() => {
                    let fields = t
                        .fields
                        .iter()
                        .map(|f| {
                            let ty = self.render_at(f.type_oid, depth + 1, false);
                            let document = t.relid.filter(|_| stored).and_then(|r| self.documents.get(&(r, f.attnum)));
                            let values = t.relid.filter(|_| stored).and_then(|r| self.values.get(&(r, f.attnum)));
                            let ty = match (ty, document, values) {
                                (Type::Json { schema: None }, Some(schema), _) => Type::Json { schema: Some(schema.clone()) },
                                (Type::Scalar { name }, _, Some(values)) if (name == "text" || name == "varchar") && !values.is_empty() => Type::Enum { name, labels: values.clone() },
                                (ty, _, _) => ty,
                            };
                            Field { name: f.name.clone(), ty, nullable: !(stored && f.not_null) }
                        })
                        .collect();
                    return Type::Composite { name: t.name.clone(), fields };
                }
                // A row value whose columns were not read: no driver parses one, so it reaches
                // the caller as its text form, and so does an array of them.
                (TypeKind::Composite, _, _) => return Type::Scalar { name: "record".into() },
                (TypeKind::Base, _, _) if matches!(t.oid, oid::JSON | oid::JSONB) => return Type::Json { schema: None },
                _ => return Type::Scalar { name: t.name.clone() },
            }
        }
        Type::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Field, Type};
    use serde_json::json;

    /// A `TYPE_QUERY` row for a composite, its columns as a driver hands them back: one decodes
    /// `int8[]` as numbers, another as digit strings.
    fn users_row(oids: Value) -> Vec<Value> {
        vec![json!(90001), json!("users"), json!("c"), json!(0), json!(0), Value::Null, json!("users"), json!(["id", "name", "tags"]), oids, json!([true, false, false])]
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
            let users = Type::Composite {
                name: "users".into(),
                fields: vec![
                    Field { name: "id".into(), ty: Type::Scalar { name: "int8".into() }, nullable: false },
                    Field { name: "name".into(), ty: Type::Scalar { name: "text".into() }, nullable: true },
                    Field { name: "tags".into(), ty: Type::Array { of: Box::new(Type::Scalar { name: "text".into() }), nullable_items: None, delimiter: None }, nullable: true },
                ],
            };
            // A stored row of the table holds its NOT NULL; a row value of its type need not.
            assert_eq!(registry.render_row(90001), users);
            let Type::Composite { name, fields } = users else { unreachable!() };
            let built = Type::Composite { name, fields: fields.into_iter().map(|f| Field { nullable: true, ..f }).collect() };
            assert_eq!(registry.render(90001), built);
        }
    }

    #[test]
    fn a_composite_without_its_columns_stays_a_record() {
        let mut registry = TypeRegistry::with_builtins();
        // The seven columns a host that predates the composite columns answers with.
        registry.insert_rows(&[vec![json!(90001), json!("users"), json!("c"), json!(0), json!(0), Value::Null, json!("users")]]);
        assert!(registry.get(90001).unwrap().fields.is_empty());
        assert_eq!(registry.render(90001), Type::Scalar { name: "record".into() });
        // `record` itself is a pseudo-type with no columns.
        assert_eq!(registry.render(2249), Type::Scalar { name: "record".into() });
    }

    #[test]
    fn missing_follows_a_composites_columns() {
        let mut registry = TypeRegistry::with_builtins();
        // `id` is a domain the registry has not seen, `tags` an array of an enum it has not seen.
        let row = vec![json!(90001), json!("users"), json!("c"), json!(0), json!(0), Value::Null, json!("users"), json!(["id", "tags"]), json!([90002, 90003]), json!([true, false])];
        registry.insert_rows(&[row]);
        let mut out = Vec::new();
        registry.missing(90001, &mut out);
        assert_eq!(out, vec![90002, 90003]);
        registry.insert_rows(&[
            vec![json!(90002), json!("user_id"), json!("d"), json!(0), json!(20), Value::Null, json!("user_id")],
            vec![json!(90003), json!("_mood"), json!("b"), json!(90004), json!(0), Value::Null, json!("mood[]")],
        ]);
        let mut out = Vec::new();
        registry.missing(90001, &mut out);
        assert_eq!(out, vec![90004]);
        registry.insert_rows(&[vec![json!(90004), json!("mood"), json!("e"), json!(0), json!(0), json!(["ok", "sad"]), json!("mood")]]);
        let mut out = Vec::new();
        registry.missing(90001, &mut out);
        assert!(out.is_empty());
        let Type::Composite { fields, .. } = registry.render(90001) else { panic!("a composite") };
        assert_eq!(fields[0].ty, Type::Scalar { name: "int8".into() });
        assert_eq!(fields[1].ty, Type::Array { of: Box::new(Type::Enum { name: "mood".into(), labels: vec!["ok".into(), "sad".into()] }), nullable_items: None, delimiter: None });
    }

    #[test]
    fn an_array_carries_its_elements_delimiter_where_it_is_not_a_comma() {
        let mut registry = TypeRegistry::with_builtins();
        // The thirteen columns of TYPE_QUERY: PostGIS's geometry, its array, a domain over it and
        // the domain's array. A domain has its base type's delimiter.
        let row = |oid: u32, name: &str, typtype: &str, elem: u32, base: u32, delimiter: &str| {
            vec![json!(oid), json!(name), json!(typtype), json!(elem), json!(base), Value::Null, json!(name), Value::Null, Value::Null, Value::Null, Value::Null, Value::Null, json!(delimiter)]
        };
        registry.insert_rows(&[
            row(90001, "geometry", "b", 0, 0, ":"),
            row(90002, "_geometry", "b", 90001, 0, ","),
            row(90003, "place", "d", 0, 90001, ":"),
            row(90004, "_place", "b", 90003, 0, ","),
        ]);
        let geometry = Box::new(Type::Scalar { name: "geometry".into() });
        assert_eq!(registry.render(90002), Type::Array { of: geometry.clone(), nullable_items: None, delimiter: Some(":".into()) });
        assert_eq!(registry.render(90004), Type::Array { of: geometry, nullable_items: None, delimiter: Some(":".into()) });
        // A comma, and a host that predates the column, leave it out.
        assert_eq!(registry.render(1009), Type::Array { of: Box::new(Type::Scalar { name: "text".into() }), nullable_items: None, delimiter: None });
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
