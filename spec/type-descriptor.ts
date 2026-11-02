/** The result of introspecting one SQL statement (`StatementDescriptor`, below) and the type descriptor of each parameter and column in it: which values the position can hold, across relational databases. Every kind, from everyday to advanced; a union lists its members in this order, and a new kind is appended, so the order never changes. The TypeScript is notation: a descriptor is data, written as JSON, YAML or anything else that carries objects, lists, strings, numbers and booleans.
 *
 * The spec is a living document, and every part of it was weighed against the databases it covers. A change to it is a design decision: agree it with a maintainer before writing it, and where implementing it shows a limitation, raise the limitation rather than working around it. */
export type Type =
  | BooleanType | IntegerType | FloatType | DecimalType
  | TextType | BytesType | UuidType
  | ArrayType | RecordType | JsonType
  | DateType | TimeType | DateTimeType | InstantType | DurationType
  | UnionType | SetType | MapType | RangeType | MultirangeType | GeometryType | XmlType
  | OpaqueType | AnyType | UnknownType;

/** What every type may carry. */
interface Base {
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
interface Bounds {
  min?: string;
  max?: string;
  exclusive_min?: string;
  exclusive_max?: string;
}

/** How long a value is, in whole numbers of its kind's unit (bytes, elements, or a text's own `unit`): `exact`, or `min`, `max` or both, never `exact` beside either. `char(8)` is `size: { exact: 8 }`, `varchar(255)` is `size: { max: 255 }`, and Oracle's `VARCHAR2(20 CHAR)`, which stores `''` as null, is `size: { min: 1, max: 20 }`. A length is never exclusive, since a whole number has an inclusive spelling. */
interface Size {
  exact?: number;
  min?: number;
  max?: number;
}

interface Lengths {
  size?: Size;
}

// Truth values and numbers -------------------------------------------------------------------

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

// Text and bytes -----------------------------------------------------------------------------
//
// Text is one kind in three shapes, by whether every value is valid Unicode (well-formed, in the Unicode Standard's word, which JavaScript's `isWellFormed()` uses too). A length counts in a unit, since languages disagree on what a string's length is: four code points take 4 to 16 bytes in UTF-8, and a JavaScript string's `length` counts UTF-16 units. A shape takes only the units that mean something for its values. How text compares, under a collation, is not part of its type, since a collation adds and removes no value.

export type TextType = UnicodeText | IllFormedUtf8Text | IllFormedUtf16Text;

/** Valid Unicode: Postgres, MySQL and Oracle text. Its lengths count code points (`varchar(255)` is `size: { max: 255 }`), UTF-16 units (Oracle `NVARCHAR2`), or bytes in the encoding the database stores it in (MySQL `TEXT` is `size: { unit: "byte", encoding: "UTF-8", max: 65535 }`). `char(8)` is `size: { exact: 8 }`, since Postgres pads it with spaces. */
export interface UnicodeText extends Base, TextFacts {
  kind: "text";
  ill_formed?: false;
  size?: TextSize;
}

/** A valid text's size, which says what it counts: code points when `unit` is absent, UTF-16 units, or bytes in `encoding`. */
interface TextSize extends Size {
  unit?: "utf16_unit" | "byte";
  /** The encoding bytes are counted in, written only with `unit: "byte"`. */
  encoding?: string;
}

/** Bytes meant as UTF-8 but never validated, so a value may not be valid Unicode: SQLite `TEXT` (`CAST(x'ff' AS TEXT)`), ClickHouse `String`, a Postgres `SQL_ASCII` database, Firebird `NONE`. A JavaScript string cannot hold every value. Its lengths count bytes, and its patterns are `like` and `ilike`, since a regular expression matches characters. */
export interface IllFormedUtf8Text extends Base, Lengths {
  kind: "text";
  ill_formed: "utf8";
  pattern?: { dialect: "like" | "ilike"; source: string };
  enum?: string[];
}

/** UTF-16 units that may hold a lone surrogate: SQL Server `nvarchar`, `nchar` and `ntext`. A Rust `String` cannot hold every value. Its lengths count UTF-16 units. */
export interface IllFormedUtf16Text extends Base, TextFacts, Lengths {
  kind: "text";
  ill_formed: "utf16";
}

interface TextFacts {
  /** A pattern every value matches, in its own dialect and matched as that dialect matches: `like` and `ilike` against the whole value, a regular expression anywhere in it unless anchored. A case-insensitive match (`~*`, or `REGEXP` under a case-insensitive collation) begins its source with `(?i)`. Postgres's and ECMA's regular expressions disagree on `.` and newlines. */
  pattern?: { dialect: "postgres" | "mysql" | "posix" | "ecma" | "like" | "ilike"; source: string };
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

// Structure ----------------------------------------------------------------------------------

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
 *   - `schema`: a JSON Schema every value satisfies, from a CHECK or a parameter's typed cast, with the draft it is read by: MySQL's `JSON_SCHEMA_VALID` reads draft 4, and Postgres's validator extensions differ.
 *   - Neither: any JSON, whose numbers may have any length (`jsonb` keeps them as `numeric`), never only doubles.
 *
 * What each database writes for each type is in "Facts about each database".
 */
export interface JsonType extends Base {
  kind: "json";
  content?: Position;
  schema?: { dialect: string; source: object };
}

// Dates and times ----------------------------------------------------------------------------
//
// One kind per class every language has: a date, a time of day, a wall-clock date and time, an instant, and a duration. Each but the time of day may hold infinity and -infinity too, as Postgres's do. Wall-clock time is never an instant: a time with no zone cannot be read as UTC. `tick` is the smallest step, in seconds, in lowest terms: `"1/1000000"` (microseconds), `"1/300"` (SQL Server `datetime`), `"60"` (`smalldatetime`). A date-bearing kind's range is 0001-01-01 to 9999-12-31, and a time's 00:00:00 up to 24:00:00, unless its bounds say otherwise.

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
type Zone = "offset" | "region" | "region_or_offset";

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
  months?: { max?: string };
  days?: { max?: string };
  time?: { tick: string; max?: string };
  /** Also holds infinity and -infinity: Postgres's, from version 17. */
  infinity?: true;
}

// Advanced -----------------------------------------------------------------------------------

/**
 * One of several types.
 *   - `per: "value"`: each value is one of them. A SQLite column that is not STRICT holds what its affinity stores, and an object column with subtypes holds any subtype's record.
 *   - `per: "execution"`: every value of one execution is the same one. MySQL's `:n + 1` is a BIGINT, a DOUBLE or a DECIMAL, by what is bound.
 *
 * A union of no members holds no value, so a position typed so with `nullable` is always null: the field of a `json_build_object('a', null)`.
 */
export interface UnionType extends Base {
  kind: "union";
  members: Type[];
  per: "value" | "execution";
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
type OrderedType = IntegerType | FloatType | DecimalType | TextType | BytesType | UuidType | DateType | TimeType | DateTimeType | InstantType | DurationType;

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

// Escape hatches ----------------------------------------------------------------------------

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

// =============================================================================================
// The statement and its positions
// =============================================================================================

/** What introspecting one statement returns. */
export interface StatementDescriptor {
  dialect: Dialect;
  /** The statement as the database runs it, with its own placeholders (`$1` on Postgres, `?` on MySQL and SQLite), in the order `params` lists them. */
  sql: string;
  /** `other` for any statement that is none of the five (`CALL`, `VALUES`, `EXPLAIN`, a MySQL `SHOW`, DDL); absent when the statement does not parse. */
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

// =============================================================================================
// Rules
// =============================================================================================
//
// 1. **A consumer reads `kind`, the fields its kind requires (an integer's `bits`, a float's widths, a time's `tick`) and the widening fields, and may ignore the rest.** That is enough to pick a representation that holds every value it can receive. A field either widens the type or narrows it, on its own, so any combination is valid. A flag is a boolean written only when true: its default is false. The widening fields are a closed list, and a new one means a new major version, while a narrowing one may come in a minor version, since a consumer that ignores it stays sound:
//      - `nullable` and `optional` on a position;
//      - `unsigned` on an integer;
//      - `infinity` and `nan`;
//      - `zone` on a time or an instant;
//      - `ill_formed` on text;
//      - `any_depth` on an array, `duplicates` on a set, and `fragment` on XML;
//      - `months`, `days` and `time` on a duration, each a part its values may carry;
//      - a date-bearing kind's bounds beyond 0001-01-01 to 9999-12-31, and a time's reaching 24:00:00.
//
// 2. **A type is what the database guarantees**, never only what was declared: a SQLite column that is not STRICT is the union of what its affinity stores. A bound appears only when it is true of every finite value, the `infinity` and `nan` flags speaking for the rest: from a type modifier, a validated CHECK on a column read straight from its table, a domain's check, or a constant the statement writes. A constraint that is not validated is not reported, and neither is a domain's NOT NULL, which an outer join defeats. A value a database stores only under a lax setting its default refuses is not typed, and a client that meets one refuses it: MySQL's zero and partial dates (`0000-00-00`, `2026-00-15`) and the empty error value of its enums.
//
// 3. **A value is what the type denotes, never its encoding.** A decimal is a number, so `1.5` and `1.50` are one value; a `timestamptz` is one instant however a session prints it; a duration is its months, days and time, so `1 mon` and `30 days` are two values though Postgres finds them equal; an array is its elements, wherever its index starts. The format says nothing about how a driver or protocol encodes a value: decoding is the driver's.
//
// 4. **One spelling per meaning.** A modifier goes in the type's fields, a name in `name`, nullability on the position. Defaults are omitted, and so is a bound other fields imply. Unions are flat, and list their members in the order `Type` lists the kinds, and members of one kind in the order the database declares them (an object type before its subtypes; an `inet`'s IPv4 record before its IPv6 one).
//
// 5. **Values inside the format are strings** in the kind's canonical text, so a 64-bit bound or a date survives any serialization; counts (`bits`, a size, a precision or scale) are numbers. Dates and times are ISO 8601, with a sign and more than four digits where a year needs them (`-4713-11-24`, `+5874897-12-31`), which RFC 3339 cannot write. Durations are ISO 8601 durations, with a sign on each part where the parts' signs differ, as Postgres writes them (`P1M-1D`).
//
// 6. **A consumer that meets an unknown kind** reads it as `opaque` at a SQL position and as any JSON inside JSON content, and reads an unknown value of a known field the same way. It ignores a field it does not know, which can only narrow, since a widening field comes with a new major version.
//
// 7. **Validity rules**, for what no shape can rule out, each a check a validator runs: a side of a range takes one of `min` and `exclusive_min` (or `max` and `exclusive_max`), and a whole-number kind takes neither exclusive field; a minimum is at most its maximum (bounds, sizes, scales); a size's `exact` never sits beside its `min` or `max`, its `min` never equals its `max`, and `encoding` appears only with `unit: "byte"`; a tick is positive; a float's widths are positive.

// =============================================================================================
// Facts about each database
// =============================================================================================
//
// What an implementation needs, and the format does not state on every node.
//
// What a SQLite column that is not STRICT holds, by its declared type's affinity: an `INTEGER`, `NUMERIC` or untyped column any of an integer, a float, text and bytes; a `REAL` column a float, text or bytes; a `TEXT` column text or bytes. A STRICT column holds its declared type alone.
//
// Which types hold infinities and NaN: Postgres's floats (both), `numeric` (NaN, and with no size the infinities), `date`, `timestamp`, `timestamptz` and, from version 17, `interval`; SQLite's `REAL` holds the infinities and stores a NaN as NULL. MySQL has none, refusing NaN and the infinities even in `FLOAT` and `DOUBLE`.
//
// What a database writes inside its own JSON:
//   - Postgres:
//       - a `numeric` is a JSON number of any length, and a NaN or Infinity a string;
//       - an `int8` is a JSON number that may pass 2^53;
//       - a `timestamptz` is ISO 8601 in the session's offset (the offset can carry seconds, and a BC year takes a suffix);
//       - an `hstore` is an object of nullable text;
//       - `jsonb` sorts keys and keeps the last duplicate, while `json` keeps the text as written.
//   - MySQL:
//       - a `DATETIME` or `TIMESTAMP` is a string with a space and six fractional digits;
//       - a `DECIMAL` is a number that keeps its scale;
//       - a comparison is `true` or `false`;
//       - a binary value is `"base64:type15:…"`;
//       - the last duplicate key wins.
//   - SQLite: ±Infinity is the number `9e999`, and a blob is never a JSON value.
//
// Bounds no field states:
//   - offsets: Postgres's run to ±15:59:59, SQL Server's to ±14:00;
//   - Postgres text cannot hold the NUL character, and `jsonb` refuses `\u0000`;
//   - Oracle `VARCHAR2(n CHAR)` is also capped at 4,000 bytes, which its one size, in code points, leaves out;
//   - a MySQL `latin1` or `utf8mb3` column holds only the characters its character set can store.

// =============================================================================================
// Examples
// =============================================================================================
//
// Checked against the types above by `nub run typecheck`, which includes this directory.

const int8: IntegerType = { kind: "integer", bits: 64, name: "int8" };

/** A first example, on Postgres, over `create table users (id int8 primary key, email varchar(255) not null, bio text); comment on column users.bio is 'Shown on the profile page';`:
 *
 *   -- One user, by id.
 *   select id, email, bio from users where id = :id
 */
export const first_example: StatementDescriptor = {
  dialect: { name: "postgres", version: "18.1" },
  sql: "select id, email, bio from users where id = $1",
  kind: "select",
  comment: "One user, by id.",
  params: [{ name: "id", type: int8 }],
  columns: [
    { name: "id", type: int8 },
    { name: "email", type: { kind: "text", size: { max: 255 }, name: "varchar" } },
    { name: "bio", nullable: true, type: { kind: "text", name: "text" }, comment: "Shown on the profile page" },
  ],
  cardinality: "at_most_one",
  diagnostics: [],
};

const int64: IntegerType = { kind: "integer", bits: 64 };
const float8: FloatType = { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true, nan: true, name: "float8" };
const timestamptz: InstantType = { kind: "instant", tick: "1/1000000", min: "-4713-11-24T00:00:00Z", max: "+294276-12-31T23:59:59.999999Z", infinity: true, name: "timestamptz" };
const point: RecordType = { kind: "record", name: "point", fields: [{ name: "x", type: float8 }, { name: "y", type: float8 }] };
const sqlite_text: TextType = { kind: "text", ill_formed: "utf8" };

/**
 * Postgres.
 *
 *   create type mood as enum ('sad', 'ok', 'happy');
 *   create domain email as varchar(255) check (value like '%@%');
 *   create table t (price numeric(10,2), total numeric, at timestamptz, day date, span interval, m mood, e email, tags varchar(20)[], flags bit(7), emb vector(3), body jsonb, during tstzrange, pos point, ring circle, addr inet, tod time);
 */
export const postgres_columns: Column[] = [
  { name: "price", nullable: true, type: { kind: "decimal", precision: 10, min_scale: 2, max_scale: 2, nan: true, name: "numeric" } },
  { name: "total", nullable: true, type: { kind: "decimal", min_scale: 0, max_scale: 16383, exclusive_min: "-1e131072", exclusive_max: "1e131072", infinity: true, nan: true, name: "numeric" } },
  { name: "at", nullable: true, type: timestamptz },
  { name: "day", nullable: true, type: { kind: "date", min: "-4713-11-24", max: "+5874897-12-31", infinity: true, name: "date" } },
  { name: "span", nullable: true, type: { kind: "duration", months: { max: "P2147483648M" }, days: { max: "P2147483648D" }, time: { tick: "1/1000000", max: "PT2562047788H54.775808S" }, infinity: true, name: "interval" } },
  { name: "m", nullable: true, type: { kind: "text", enum: ["sad", "ok", "happy"], name: "public.mood" } },
  { name: "e", nullable: true, type: { kind: "text", size: { max: 255 }, pattern: { dialect: "like", source: "%@%" }, name: "public.email" } },
  { name: "tags", nullable: true, type: { kind: "array", any_depth: true, name: "_varchar", element: { nullable: true, type: { kind: "text", size: { max: 20 }, name: "varchar" } } } },
  { name: "flags", nullable: true, type: { kind: "array", size: { exact: 7 }, name: "bit", element: { type: { kind: "boolean" } } } },
  { name: "emb", nullable: true, type: { kind: "array", size: { exact: 3 }, name: "public.vector", element: { type: { kind: "float", exponent_bits: 8, mantissa_bits: 23 } } } },
  { name: "body", nullable: true, type: { kind: "json", name: "jsonb" } },
  { name: "during", nullable: true, type: { kind: "range", subtype: timestamptz, name: "tstzrange" } },
  { name: "pos", nullable: true, type: point },
  { name: "ring", nullable: true, type: { kind: "record", name: "circle", fields: [{ name: "center", type: point }, { name: "radius", type: float8 }] } },
  {
    name: "addr", nullable: true,
    type: { kind: "union", per: "value", name: "inet", members: [
      { kind: "record", fields: [{ name: "address", type: { kind: "bytes", size: { exact: 4 } } }, { name: "prefix", type: { kind: "integer", bits: 8, unsigned: true, max: "32" } }] },
      { kind: "record", fields: [{ name: "address", type: { kind: "bytes", size: { exact: 16 } } }, { name: "prefix", type: { kind: "integer", bits: 8, unsigned: true, max: "128" } }] },
    ] },
  },
  { name: "tod", nullable: true, type: { kind: "time", tick: "1/1000000", max: "24:00:00", name: "time" } },
];

/** Postgres: `select json_agg(p.title) filter (where p.id is not null) as titles from users u left join posts p on p.author_id = u.id group by u.id`, over a NOT NULL `title`. */
export const json_column: Column = {
  name: "titles",
  nullable: true,
  type: { kind: "json", name: "json", content: { type: { kind: "array", element: { type: { kind: "text", name: "text" } } } } },
};

/**
 * MySQL.
 *
 *   create table m (id bigint unsigned primary key, flag boolean not null, at timestamp(3) null, status enum('b','a') not null, born date, name varchar(100), body text, tags set('a','b'));
 *   select id, flag, at, status, born, name, body, tags, :n + 1 next from m
 */
export const mysql_columns: Column[] = [
  { name: "id", type: { kind: "integer", bits: 64, unsigned: true, name: "bigint" } },
  { name: "flag", type: { kind: "integer", bits: 8, name: "tinyint(1)" } },
  { name: "at", nullable: true, type: { kind: "instant", tick: "1/1000", min: "1970-01-01T00:00:01Z", max: "2038-01-19T03:14:07.999Z", name: "timestamp" } },
  { name: "status", type: { kind: "text", enum: ["b", "a"], name: "enum" } },
  { name: "born", nullable: true, type: { kind: "date", name: "date" } },
  { name: "name", nullable: true, type: { kind: "text", size: { max: 100 }, name: "varchar" } },
  { name: "body", nullable: true, type: { kind: "text", size: { unit: "byte", encoding: "UTF-8", max: 65535 }, name: "text" } },
  { name: "tags", nullable: true, type: { kind: "set", size: { max: 2 }, name: "set", element: { type: { kind: "text", enum: ["a", "b"] } } } },
  { name: "next", nullable: true, type: { kind: "union", per: "execution", members: [int64, { kind: "float", exponent_bits: 11, mantissa_bits: 52 }, { kind: "decimal" }] } },
];

/** SQL Server: `create table s (at datetimeoffset(3), label nvarchar(10))`. `at` keeps the offset each value was written with, and `label` may hold a lone surrogate. */
export const sqlserver_columns: Column[] = [
  { name: "at", nullable: true, type: { kind: "instant", tick: "1/1000", zone: "offset", name: "datetimeoffset" } },
  { name: "label", nullable: true, type: { kind: "text", ill_formed: "utf16", size: { max: 10 }, name: "nvarchar" } },
];

/**
 * SQLite.
 *
 *   create table s (id integer primary key, price numeric, name text, x);
 *   create table t (n integer) strict;
 *   select price, name, x, n from s, t
 *
 * A NUMERIC column stores `'0012'` as 12, `'abc'` as text and a blob as a blob; a TEXT column stores 12 as `'12'`. In the STRICT table, `n` is an integer alone.
 */
export const sqlite_columns: Column[] = [
  { name: "price", nullable: true, type: { kind: "union", per: "value", name: "NUMERIC", members: [int64, { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true }, sqlite_text, { kind: "bytes" }] } },
  { name: "name", nullable: true, type: { kind: "union", per: "value", name: "TEXT", members: [sqlite_text, { kind: "bytes" }] } },
  { name: "x", nullable: true, type: { kind: "union", per: "value", members: [int64, { kind: "float", exponent_bits: 11, mantissa_bits: 52, infinity: true }, sqlite_text, { kind: "bytes" }] } },
  { name: "n", nullable: true, type: { ...int64, name: "INTEGER" } },
];

/** Oracle: `create type shape_t as object (name varchar2(20 char)) not final; create type circle_t under shape_t (radius number); create table shapes (s shape_t);` Oracle stores `''` as null, so its text is at least 1 code point. */
export const oracle_column: Column = {
  name: "S",
  nullable: true,
  type: { kind: "union", per: "value", name: "R.SHAPE_T", members: [
    { kind: "record", name: "R.SHAPE_T", fields: [{ name: "NAME", nullable: true, type: { kind: "text", size: { min: 1, max: 20 }, name: "VARCHAR2" } }] },
    { kind: "record", name: "R.CIRCLE_T", fields: [
      { name: "NAME", nullable: true, type: { kind: "text", size: { min: 1, max: 20 }, name: "VARCHAR2" } },
      { name: "RADIUS", nullable: true, type: { kind: "decimal", precision: 40, exclusive_min: "-1e126", exclusive_max: "1e126", name: "NUMBER" } },
    ] },
  ] },
};

/** A structured parameter: `insert into events (kind, at) select * from :events::rows(kind varchar(32), at timestamptz?)` */
export const structured_param: Param = {
  name: "events",
  type: { kind: "array", element: { type: { kind: "record", fields: [
    { name: "kind", type: { kind: "text", size: { max: 32 }, name: "varchar" } },
    { name: "at", nullable: true, optional: true, type: timestamptz },
  ] } } },
};
