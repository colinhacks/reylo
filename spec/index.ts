// Generated from typedefs.schema.json, the format's source of truth, by `nub run typedefs`: edit the
// schema, not this file. The rules every descriptor follows are in README.md beside it.

/** A type descriptor: which values a position can hold. Every kind, from everyday to advanced; a union lists its members in this order, and a new kind is appended, so the order never changes. */
export type Type =
  | BooleanType
  | IntegerType
  | FloatType
  | DecimalType
  | TextType
  | BytesType
  | UuidType
  | ArrayType
  | RecordType
  | JsonType
  | DateType
  | TimeType
  | DateTimeType
  | InstantType
  | DurationType
  | UnionType
  | SetType
  | MapType
  | RangeType
  | MultirangeType
  | GeometryType
  | XmlType
  | OpaqueType
  | AnyType
  | UnknownType
  | TaggedUnionType;

/** What every type may carry. */
export interface Base {
  /**
   * The database's name for the type, less the modifiers a field states: `numeric` for `numeric(10,2)`, but `tinyint(1)` for MySQL's `BOOLEAN`, whose `(1)` bounds no value. It is in the dialect the document states once at its top. When a schema defines the type, the name is qualified, and it is the type's identity.
   *   - Postgres: `int4`, `varchar`, `timestamptz`, `public.mood` (an enum), `public.email` (a domain), `public.vector` (an extension's type).
   *   - MySQL: `bigint`, `enum`, `json`.
   *   - SQLite: the declared type the value traces to, through `max`, `coalesce` and union arms too.
   *   - Oracle: `VARCHAR2`, `R.SHAPE_T`.
   *
   * Absent on a type the statement builds, such as the record inside a `json_build_object`. It never changes what the type means. Generators and drivers key their conventions on it, such as MySQL's `tinyint(1)` as a boolean.
   */
  name?: string;
  /** The comment the database keeps on a named type: `COMMENT ON TYPE` or `COMMENT ON DOMAIN`. A column's own comment is on the column. */
  comment?: string;
}

/** Bounds on an ordered kind's values, in its canonical text (`"-1.50"`, `"2026-10-23"`, and for a decimal an exponent where it is shorter, `"1e126"`), as JSON Schema's `minimum`, `maximum`, `exclusiveMinimum` and `exclusiveMaximum` are: each holds a value, so `CHECK (x > 0)` on a float is `exclusive_min: "0"`. A side takes one of its two fields. A whole-number kind takes only `min` and `max` (`x > 0` is `min: "1"`); any other kind writes a bound as the constraint states it, so `x > 0` on a `numeric(10,2)` is `exclusive_min: "0"`, never `min: "0.01"`. Bounds constrain the finite values only: whether infinity, -infinity and NaN also occur is said by the `infinity` and `nan` flags alone, whatever the bounds. */
export interface Bounds {
  min?: string;
  max?: string;
  exclusive_min?: string;
  exclusive_max?: string;
}

/** How long a value is, in whole numbers of its kind's unit (bytes, elements, or a text's own `unit`): `exact`, or `min`, `max` or both, never `exact` beside either. `char(8)` is `size: { exact: 8 }`, `varchar(255)` is `size: { max: 255 }`, and Oracle's `VARCHAR2(20 CHAR)`, which stores `''` as null, is `size: { min: 1, max: 20 }`. A length is never exclusive, since a whole number has an inclusive spelling. */
export interface Size {
  exact?: number;
  min?: number;
  max?: number;
}

export interface Lengths {
  size?: Size;
}

/** A truth value: Postgres `boolean`, SQL Server `bit`, Oracle `BOOLEAN`, and a comparison's result on any database, which MySQL and SQLite deliver as 1 or 0. A MySQL `BOOLEAN` column is an `integer`, since it can hold 5. */
export interface BooleanType extends Base {
  kind: "boolean";
}

/** A whole number of a fixed width, two's complement unless `unsigned`: `int4` is 32 bits, MySQL `BIGINT UNSIGNED` 64 bits unsigned, SQL Server `tinyint` 8 bits unsigned, MySQL `MEDIUMINT` 24 bits, DuckDB `HUGEINT` 128 bits, SQLite `INTEGER` 64 bits, and Vertica's `TINYINT`, despite its name, 64 bits. Its range follows from the width; bounds narrow it further, from a CHECK or for a type narrower than any width, as MySQL `YEAR` (0 and 1901 to 2155, 16 bits bounded at 0 and 2155). A whole-number bound is always written inclusive, since `x > 0` is `x >= 1`. An integer of no fixed width is a `decimal` whose scale is at most 0. */
export interface IntegerType extends Base, Bounds {
  kind: "integer";
  bits: number;
  /** From 0 to 2^bits − 1: MySQL `UNSIGNED`, SQL Server `tinyint`, Postgres `oid`. */
  unsigned?: true;
  multiple_of?: string;
  /** The only values, as JSON Schema's `enum`: a `CHECK (x IN (…))`, or a constant the statement writes (`select 1 as one`). */
  enum?: string[];
}

/** An IEEE 754 binary float, given by its exponent and mantissa widths, which fix its range and precision: `float8`, SQLite `REAL` and SQL Server `float` are 11 and 52, `float4` 8 and 23, the half float in pgvector's `halfvec` 5 and 10, and ClickHouse's `BFloat16` 8 and 7. A width alone would not do, since those last two are both 16 bits. A generator picks the smallest native float whose widths are both at least these. -0 is a float like any other, and equal to 0. */
export interface FloatType extends Base, Bounds {
  kind: "float";
  exponent_bits: number;
  /** Not counting the implicit leading bit. */
  mantissa_bits: number;
  /** Also holds infinity and -infinity: Postgres's floats, SQLite's `REAL`. MySQL's `FLOAT` and `DOUBLE` refuse them. */
  infinity?: true;
  /** Also holds NaN: Postgres's floats. SQLite stores a NaN as NULL, and MySQL refuses it. */
  nan?: true;
}

/**
 * An exact decimal: `numeric`, MySQL `DECIMAL`, Oracle `NUMBER`, `DECFLOAT`. It is a kind of its own, so that a generator switching on the kind never turns money into `int64` or `double`.
 *
 * A decimal is a coefficient, a whole number, times a power of ten: 1.25 is 125 × 10^-2. IEEE 754 and the General Decimal Arithmetic specification call the coefficient's digits the precision and the power the exponent; SQL and Java's `BigDecimal` count the exponent negated, as the scale, the digits after the point. A type bounds both:
 *   - `numeric(10,2)`: precision 10, scale 2, so up to 99,999,999.99. MySQL `DECIMAL(M,D)`, SQL Server `decimal(p,s)` and Oracle `NUMBER(p,s)` alike, and SQL Server `money` is (19,4) with bounds at -922,337,203,685,477.5808 and 922,337,203,685,477.5807. Oracle's `NUMBER(5,-2)` has scale -2, rounding to hundreds.
 *   - Postgres `numeric` with no size: scale 0 to 16,383, below 10^131072 in magnitude.
 *   - `DECFLOAT(34)`, IEEE's decimal128: precision 34, scale -6111 to 6176.
 *   - Oracle `NUMBER`: precision 40, below 10^126 in magnitude; Oracle `INTEGER` adds a scale of at most 0.
 *
 * A decimal is a number, so 1.5 and 1.50 are one value however the database prints them. Postgres `money` is `opaque`: its scale follows the session's locale, and the client receives `$1,234.57`.
 */
export interface DecimalType extends Base, Bounds {
  kind: "decimal";
  /** The most digits in the coefficient. */
  precision?: number;
  /** A value is in the type when it can be written as a coefficient of at most `precision` digits times 10^-scale, with a scale from `min_scale` to `max_scale`: so `numeric(5,2)`, precision 5 and scale 2 to 2, holds 1.5, written 150 × 10^-2. Equal bounds make a fixed scale. */
  min_scale?: number;
  max_scale?: number;
  /** Also holds infinity and -infinity: Postgres `numeric` with no size, `DECFLOAT`. Java's `BigDecimal`, Rust's `rust_decimal` and C#'s `decimal` hold neither, nor NaN. */
  infinity?: true;
  /** Also holds NaN: Postgres `numeric`, sized or not, `DECFLOAT`. */
  nan?: true;
  multiple_of?: string;
  enum?: string[];
}

/** Text is one kind in three shapes, by whether every value is valid Unicode (well-formed, in the Unicode Standard's word, which JavaScript's `isWellFormed()` uses too). A length counts in a unit, since languages disagree on what a string's length is: four code points take 4 to 16 bytes in UTF-8, and a JavaScript string's `length` counts UTF-16 units. A shape takes only the units that mean something for its values. How text compares, under a collation, is not part of its type, since a collation adds and removes no value. */
export type TextType = UnicodeText | IllFormedUtf8Text | IllFormedUtf16Text;

/** Valid Unicode: Postgres, MySQL and Oracle text. Its lengths count code points (`varchar(255)` is `size: { max: 255 }`), UTF-16 units (Oracle `NVARCHAR2`), or bytes in the encoding the database stores it in (MySQL `TEXT` is `size: { unit: "byte", encoding: "UTF-8", max: 65535 }`). `char(8)` is `size: { exact: 8 }`, since Postgres pads it with spaces. */
export interface UnicodeText extends Base, TextFacts {
  kind: "text";
  ill_formed?: false;
  size?: TextSize;
}

/** A valid text's size, which says what it counts: code points when `unit` is absent, UTF-16 units, or bytes in `encoding`. */
export interface TextSize extends Size {
  unit?: "utf16_unit" | "byte";
  /** The encoding bytes are counted in, written only with `unit: "byte"`. */
  encoding?: string;
}

/** Bytes meant as UTF-8 but never validated, so a value may not be valid Unicode: SQLite `TEXT` (`CAST(x'ff' AS TEXT)`), ClickHouse `String`, a Postgres `SQL_ASCII` database, Firebird `NONE`. A JavaScript string cannot hold every value. Its lengths count bytes, and its patterns are `like` and `ilike`, since a regular expression matches characters. */
export interface IllFormedUtf8Text extends Base, Lengths {
  kind: "text";
  ill_formed: "utf8";
  pattern?: { dialect: "like" | "ilike"; source: string; };
  enum?: string[];
}

/** UTF-16 units that may hold a lone surrogate: SQL Server `nvarchar`, `nchar` and `ntext`. A Rust `String` cannot hold every value. Its lengths count UTF-16 units. */
export interface IllFormedUtf16Text extends Base, TextFacts, Lengths {
  kind: "text";
  ill_formed: "utf16";
}

export interface TextFacts {
  /** A pattern every value matches, in its own dialect and matched as that dialect matches: `like` and `ilike` against the whole value, a regular expression anywhere in it unless anchored. A case-insensitive match (`~*`, or `REGEXP` under a case-insensitive collation) begins its source with `(?i)`. Postgres's and ECMA's regular expressions disagree on `.` and newlines. */
  pattern?: { dialect: "postgres" | "mysql" | "posix" | "ecma" | "like" | "ilike"; source: string; };
  /** The only values, as JSON Schema's `enum`: a database's enum type (Postgres `create type mood as enum (…)`, MySQL `ENUM(…)`), a `CHECK (x IN (…))`, or a constant the statement writes (`select 'user' as kind`). An enum type's declared order concerns only how its values sort, so it is not part of the type. */
  enum?: string[];
}

/** Bytes: `bytea`, `binary(n)` (exactly n), `varbinary(n)`, `BLOB`, `RAW(n)`, SQL Server `rowversion` (8), Postgres `macaddr` (6) and `macaddr8` (8). */
export interface BytesType extends Base, Lengths {
  kind: "bytes";
}

/** A 128-bit identifier (RFC 9562): Postgres `uuid`, SQL Server `uniqueidentifier`, MariaDB `UUID`. */
export interface UuidType extends Base {
  kind: "uuid";
}

/**
 * A sequence of `element`s, in order. Where its index starts is not part of the type.
 *   - **Nesting:** a two-dimensional array is an array of arrays. A Postgres array column takes `any_depth`, since Postgres ignores declared dimensions: one `int[]` column holds `{1,2}` and `{{1,2},{3,4}}`. Every array a statement builds (`array_agg`, `ARRAY[…]`, a JSON array) has the depth its nesting shows.
 *   - **Its size** counts the outermost array's elements. A fixed shape, such as DuckDB's `INTEGER[3]` or a 3×3 matrix, is arrays of exact sizes, nested.
 *   - **A vector** is an array of floats: pgvector's `vector(3)` is exactly 3 elements, and MySQL's `VECTOR(n)` at most n.
 *   - **A bit string** is an array of booleans: Postgres `bit(7)` and MySQL `BIT(7)` are exactly 7, and Postgres `varbit(n)` at most n. That is its value, not its representation: a client may hold one as a string of 0s and 1s, a number or a buffer.
 */
export interface ArrayType extends Base, Lengths {
  kind: "array";
  element: Position;
  /** An element may itself be an array of `element`, to any depth, each value rectangular. */
  any_depth?: true;
}

/**
 * Fields, in order: a composite type, a row value, an object type, a JSON object a statement builds, or a structured parameter's row. A database's sugar over simpler values is a record carrying its `name`:
 *   - a Postgres `point` is `{ x, y }` and a `circle` `{ center, radius }`;
 *   - an `inet` is a union of an IPv4 record, `{ address, prefix }` with 4 bytes and a prefix to 32, and an IPv6 one, with 16 bytes and a prefix to 128; a `cidr` likewise, its host bits zero;
 *   - an Oracle `BFILE` is `{ directory, file }`.
 *
 * A record holds exactly its fields. A column whose object type has subtypes is a union of their records.
 */
export interface RecordType extends Base {
  kind: "record";
  fields: Field[];
}

/**
 * A JSON value: `json`, `jsonb`, MySQL `JSON`.
 *   - `content`: what the statement built (`json_build_object`, `json_agg`), described with the same types. Each leaf is typed by the SQL value the database wrote it from: inside a `json_build_object`, an `int8` may pass 2^53, which a JSON Schema could not say.
 *   - `schema`: a JSON Schema every value satisfies, from a CHECK or a parameter's typed cast, whose `$schema` names the draft it is read by: MySQL's `JSON_SCHEMA_VALID` reads draft 4, and Postgres's validator extensions differ.
 *   - Neither: any JSON, whose numbers may have any length (`jsonb` keeps them as `numeric`), never only doubles.
 *
 * What each database writes for each type is in "Facts about each database".
 */
export interface JsonType extends Base {
  kind: "json";
  content?: Position;
  schema?: { $schema: string; [keyword: string]: unknown; };
}

/** A calendar date. Postgres's runs from `-4713-11-24` to `+5874897-12-31`, in astronomical years. */
export interface DateType extends Base, Bounds {
  kind: "date";
  /** Also holds infinity and -infinity, which sort after and before every other value, whatever the bounds, which constrain only finite values. */
  infinity?: true;
}

/** A time of day: `time(p)`. Postgres's holds `24:00:00` itself, its `max`, which Java's `LocalTime`, Python's `time` and Temporal refuse. With `zone`, each value keeps an offset too: Postgres `timetz`. */
export interface TimeType extends Base, Bounds {
  kind: "time";
  tick: string;
  zone?: Zone;
}

/** A date and time of day as a wall clock reads, with no zone: Postgres `timestamp`, MySQL `DATETIME`, SQL Server `datetime2`, Oracle `DATE`. */
export interface DateTimeType extends Base, Bounds {
  kind: "datetime";
  tick: string;
  /** Also holds infinity and -infinity, which sort after and before every other value, whatever the bounds, which constrain only finite values. */
  infinity?: true;
}

/** A point on the timeline: Postgres `timestamptz`, MySQL `TIMESTAMP`, Oracle `TIMESTAMP WITH LOCAL TIME ZONE`. Each stores the moment and shows it in the session's zone. Bounds are in UTC. */
export interface InstantType extends Base, Bounds {
  kind: "instant";
  tick: string;
  /** Absent: only the moment, with no claim about any offset, as Postgres `timestamptz`, which takes `12:00+05:30` and gives back that moment in the session's zone. */
  zone?: Zone;
  /** Also holds infinity and -infinity, which sort after and before every other value, whatever the bounds, which constrain only finite values. */
  infinity?: true;
}

/** What each value keeps besides the moment: a UTC offset (SQL Server `datetimeoffset`, Postgres `timetz`), a region such as `America/Los_Angeles`, or either, per value (Oracle `TIMESTAMP WITH TIME ZONE`, Firebird's `WITH TIME ZONE` types). No database keeps regions alone today. */
export type Zone = "offset" | "region" | "region_or_offset";

/**
 * A length of time, in the parts it has, each signed on its own. A month and a day have no fixed length (a month is 28 to 31 days, and a day 23 to 25 hours across a clock change), so a duration that counts them keeps them apart from its time: Postgres adds `1 month` to January 31 as February 28, and `1 day` across a clock change as 23 or 25 hours.
 *   - Postgres `interval` has all three (`1 mon -1 day`, which `Temporal.Duration` refuses).
 *   - Oracle's `INTERVAL YEAR TO MONTH` has `months`, and its `INTERVAL DAY TO SECOND` `time`, since it holds a day as 24 hours.
 *   - MySQL `TIME` is a `time` part, up to 838:59:59.
 *
 * A part's `max` bounds its magnitude, of either sign, as an ISO 8601 duration (`P2147483648M`, `PT838H59M59S`). A two's complement part reaches one unit further below zero than above it, so its `max` is the negative side's magnitude: Postgres's `months` reaches -2147483648 and +2147483647, and is `P2147483648M`. SQL calls this kind an interval; ISO 8601's intervals are a start and an end, which is a `range` here.
 */
export interface DurationType extends Base {
  kind: "duration";
  months?: { max?: string; };
  days?: { max?: string; };
  time?: { tick: string; max?: string; };
  /** Also holds infinity and -infinity: Postgres's, from version 17. */
  infinity?: true;
}

/**
 * One of several types: each value is one of them. A SQLite column that is not STRICT holds what its affinity stores, an object column with subtypes holds any subtype's record, and MySQL's `:n + 1` is a BIGINT, a DOUBLE or a DECIMAL by what is bound and how the driver sends it. A union whose members a database names, and whose values say which member they are, is a `tagged_union`.
 *
 * A union of no members holds no value, so a position typed so with `nullable` is always null: the field of a `json_build_object('a', null)`.
 */
export interface UnionType extends Base {
  kind: "union";
  members: Type[];
}

/** Elements in no order, each at most once by the element type's equality: MySQL `SET`, a set of the values it declares, at most 64. A client may receive the elements in any order. Not an array, since it has no rank and no index. */
export interface SetType extends Base, Lengths {
  kind: "set";
  element: Position;
  /** An element may appear more than once, making a multiset: the SQL standard's `MULTISET`, Oracle's nested tables. */
  duplicates?: true;
}

/** Keys to values, each key once by the key type's equality, in no order: `hstore` (text to nullable text). A key is never null. A map whose keys may repeat, such as ClickHouse's `Map`, is an array of `{ key, value }` records. */
export interface MapType extends Base {
  kind: "map";
  key: Type;
  value: Position;
}

/** The kinds that have an order, so the only ones a range can span. */
export type OrderedType =
  | IntegerType
  | FloatType
  | DecimalType
  | TextType
  | BytesType
  | UuidType
  | DateType
  | TimeType
  | DateTimeType
  | InstantType
  | DurationType;

/** A range of `subtype`, with an empty range and unbounded ends: Postgres ranges, Teradata `PERIOD`, and ISO 8601's intervals, a start and an end. Over a discrete subtype, such as integers or dates, `[1,3]` and `[1,4)` are one range. An unbounded end lies beyond the subtype's infinities, so `[x,)` holds infinity and `[x,infinity)` does not. */
export interface RangeType extends Base {
  kind: "range";
  subtype: OrderedType;
}

/** An ordered set of non-overlapping ranges: Postgres multiranges. */
export interface MultirangeType extends Base {
  kind: "multirange";
  subtype: OrderedType;
}

/** An OGC simple feature: PostGIS `geometry` and `geography` (`geodetic`), and the spatial types of MySQL, SQL Server and Oracle. */
export interface GeometryType extends Base {
  kind: "geometry";
  model?: "planar" | "geodetic";
  srid?: number;
  subtype?: "point" | "linestring" | "polygon" | "multipoint" | "multilinestring" | "multipolygon" | "geometrycollection";
  coordinates?: "xy" | "xyz" | "xym" | "xyzm";
}

/** XML: a document with one root, valid against any `schemas` given, or a fragment, which may have several roots (`<a/><b/>`), as Postgres's `xml` may. An XML Schema validates documents only, so a fragment names none. */
export type XmlType = XmlDocument | XmlFragment;

export interface XmlDocument extends Base {
  kind: "xml";
  fragment?: false;
  schemas?: string[];
}

export interface XmlFragment extends Base {
  kind: "xml";
  fragment: true;
}

/** A type the database knows and the format does not structure: an extension's own, such as Postgres `ltree`, `tsvector` and `pg_lsn` or SQL Server's `hierarchyid`, and Postgres `money`, whose scale follows the session's locale. Its `name` is all the format says; what a value looks like is the driver's to decide. */
export interface OpaqueType extends Base {
  kind: "opaque";
  name: string;
}

/** The database's own type admits a value of any type, each carrying its type, which is one of this format's types: SQL Server `sql_variant`, Oracle `ANYDATA`. */
export interface AnyType extends Base {
  kind: "any";
}

/** Nothing is known about the values. */
export interface UnknownType {
  kind: "unknown";
}

/** One of several alternatives, each named by the tag every value carries, in the order the type declares them: DuckDB's `UNION(num INTEGER, str VARCHAR)`, a Rust enum. Unlike a union's members, two alternatives may hold one type, and a value says which alternative it is, even where it holds NULL (DuckDB's `union_value(num := NULL)` is no NULL union). */
export interface TaggedUnionType extends Base {
  kind: "tagged_union";
  alternatives: Alternative[];
}

/** An alternative of a tagged union: its tag, unique in the type, and the value it holds. */
export interface Alternative extends Position {
  tag: string;
}

/** What introspecting one statement returns. A tool may write keys of its own beside these, as `reylo analyze` writes the file a statement was read from. */
export interface StatementDescriptor {
  /** The version of the format the descriptor is written in, as the URL of that version's JSON Schema, which a validator or an editor reads. Versions are semantic, and until 1.0.0 a new minor version may change anything. */
  $schema: "https://reylo.dev/typedefs/0.1.0.json";
  dialect: Dialect;
  /** The statement as the database runs it, with its own placeholders (`$1` on Postgres, `?` on MySQL and SQLite), in the order `params` lists them. */
  sql: string;
  /** `select` for any query, a `VALUES` list and a set operation included; `other` for any statement that is none of the five: a `CALL`, whose procedure may write, an `EXPLAIN`, a `SHOW` or a SQLite `PRAGMA`, and DDL. Absent when the statement does not parse. */
  kind?: "select" | "insert" | "update" | "delete" | "merge" | "other";
  /** The prose written in comments before the statement: its summary, as an editor's hover shows it. */
  comment?: string;
  params: Param[];
  /** In order; empty when the statement returns no rows. */
  columns: Column[];
  /** How many rows the statement returns: exactly one, at most one, any number, or none (a write without `RETURNING`). */
  cardinality: "one" | "at_most_one" | "many" | "none";
  /** What the database or the introspection reports against the statement, each at its place, so an editor can underline it. A statement that does not parse has a diagnostic and no columns. */
  diagnostics: Diagnostic[];
}

/** `start` and `end` are offsets into the statement as it was written, in UTF-16 units, as the Language Server Protocol counts by default. `code` is the database's own error code where it has one (Postgres's SQLSTATE `42703`, MySQL's `1054`). */
export interface Diagnostic {
  message: string;
  start: number;
  end: number;
  code?: string;
}

/** Where a value stands: a column, a parameter, a record's field, an array's element, a map's value, or JSON content. Nullability lives here, never in a type, since an outer join makes any position nullable whatever its type. */
export interface Position {
  type: Type;
  /** The value may be null; absent, it never is. On a parameter, the statement accepts null; inside JSON content, the value may be JSON null. */
  nullable?: true;
}

/** A result's column. A row always has every column, so none is optional. Columns can share a name (`select a.id, b.id`), so a row is a list of columns, not an object. */
export interface Column extends Position {
  /** The name the database gives the column: Postgres calls `select 1`'s `?column?`, and SQL Server leaves it empty. */
  name: string;
  /** The comment the database keeps on the column (`COMMENT ON COLUMN`). */
  comment?: string;
}

/** A record's field. */
export interface Field extends Position {
  name: string;
  /** The key may be absent: a structured parameter's `?` member, a JSON object's key (as `jsonb_strip_nulls` drops). Apart from `nullable`, and never a default value. */
  optional?: true;
}

/** A statement's parameter. Where the statement writes it whole into a column, its type is that column's: `insert into t (price) values (:price)`, into a `numeric(10,2)` column, gives `price` precision 10 and scale 2, though Postgres describes the parameter as plain `numeric` and would round 1.234. That is stricter than the server, never looser. */
export interface Param extends Position {
  name: string;
  /** The caller may leave the parameter out. */
  optional?: true;
  /** The comment the database keeps on the column the parameter is written to whole. */
  comment?: string;
}

/** The database a document of descriptors describes, stated once at the document's top: every `name` inside is this dialect's. `name` is the database's, such as `postgres`, `mysql`, `sqlite`, `oracle` or `sqlserver`, and `version` its server version. */
export interface Dialect {
  name: string;
  version: string;
}
