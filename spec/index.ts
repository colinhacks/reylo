// Generated from typedefs.schema.json, the format's source of truth, by `nub run typedefs`: edit the
// schema, not this file. The rules every descriptor follows are in README.md beside it.

/** A type: which values a slot can hold. A union or an intersection lists its members in this order, and a new kind is appended, so the order never changes. */
export type Type =
  | BooleanType
  | IntegerType
  | FloatType
  | DecimalType
  | TextType
  | BytesType
  | UuidType
  | EnumType
  | DateType
  | TimeType
  | DateTimeType
  | InstantType
  | ZonedDateTimeType
  | DurationType
  | ArrayType
  | TupleType
  | StructType
  | MapType
  | TableType
  | JsonType
  | UnionType
  | IntersectionType
  | TaggedUnionType
  | RangeType
  | MultirangeType
  | GeometryType
  | XmlType
  | NullType
  | OpaqueType
  | UnknownType;

/** What every type may carry. */
export interface Base {
  /** The source's own name for the type, less the modifiers its fields state: `numeric` for `numeric(10,2)`, `varchar` for `varchar(255)`. A database's name is in the dialect the document states at its top, and a type a schema defines is qualified (`public.mood`), which makes the label its identity. Absent on a type the source builds, such as the struct of a `json_build_object`. It never changes what the type means; generators and drivers may key conventions on it. */
  label?: string;
  /** The comment the source keeps on a named type, such as `COMMENT ON TYPE`. A field's own comment is on the field. */
  comment?: string;
  /** A short name for the type, as JSON Schema's `title`. Like the comment, it says nothing of the values. */
  title?: string;
}

/** Inclusive bounds on an integer's values, in its canonical text, as JSON Schema's `minimum` and `maximum` are. Every bound on a whole number has an inclusive spelling (`x > 0` is `min: "1"`), so an integer takes no exclusive bound. Bounds narrow its values only beyond what its width implies. */
export interface InclusivelyBounded {
  min?: string;
  max?: string;
}

/** Bounds on an ordered kind's finite values, in its canonical text (`"-1.50"`, `"2026-10-23"`), as JSON Schema's `minimum`, `maximum`, `exclusiveMinimum` and `exclusiveMaximum` are. A side takes one of its two fields. A bound is written as the constraint states it, so `x > 0` on a decimal of scale 2 is `exclusive_min: "0"`, never `min: "0.01"`. Whether infinity, -infinity and NaN occur is said by the `infinity` and `nan` flags alone, whatever the bounds. */
export interface Bounded extends InclusivelyBounded {
  exclusive_min?: string;
  exclusive_max?: string;
}

/** How long a value is, in whole numbers of its kind's unit (bytes, elements, a map's keys, or a text's `unit`): `exact`, or `min`, `max` or both. A length is never exclusive, since a whole number has an inclusive spelling. */
export interface Size {
  exact?: number;
  min?: number;
  max?: number;
}

export interface Sized {
  size?: Size;
}

/** A truth value. */
export interface BooleanType extends Base {
  kind: "boolean";
}

/** A whole number of a fixed width, two's complement unless `unsigned`: 32 bits for a typical `int`, 128 for a `hugeint`. Its range follows from the width, and bounds narrow it further, as for a year held in 16 bits and kept to 1901 to 2155. An integer of no fixed width is a `decimal` whose scale is at most 0. */
export interface IntegerType extends Base, InclusivelyBounded {
  kind: "integer";
  bits: number;
  /** From 0 to 2^bits − 1. */
  unsigned?: true;
  multiple_of?: string;
}

/** An IEEE 754 binary float, given by its exponent and mantissa widths, which fix its range and precision: a double is 11 and 52, a single 8 and 23, a half 5 and 10, and a bfloat16 8 and 7. A width alone would not do, since those last two are both 16 bits. A generator picks the smallest native float whose widths are both at least these. -0 is a float like any other, and equal to 0. */
export interface FloatType extends Base, Bounded {
  kind: "float";
  exponent_bits: number;
  /** Not counting the implicit leading bit. */
  mantissa_bits: number;
  /** Also holds infinity and -infinity. */
  infinity?: true;
  /** Also holds NaN. */
  nan?: true;
}

/** An exact decimal: a coefficient, a whole number, times a power of ten (1.25 is 125 × 10^-2). It is a kind of its own, so that a generator switching on the kind never turns money into a float. SQL counts the power negated, as the scale, the digits after the point, and a type bounds both: `numeric(10,2)` has precision 10 and scale 2, so it holds up to 99,999,999.99. A decimal is a number, so 1.5 and 1.50 are one value however a source prints them. */
export interface DecimalType extends Base, Bounded {
  kind: "decimal";
  /** The most digits in the coefficient. */
  precision?: number;
  /** A value is in the type when it can be written as a coefficient of at most `precision` digits times 10^-scale, with a scale from `min_scale` to `max_scale`: precision 5 and scale 2 to 2 hold 1.5, written 150 × 10^-2. Equal bounds make a fixed scale; a negative scale rounds to tens, hundreds and so on. */
  min_scale?: number;
  max_scale?: number;
  /** Also holds infinity and -infinity. */
  infinity?: true;
  /** Also holds NaN. */
  nan?: true;
  multiple_of?: string;
}

/** Text. With no `encoding`, its values are any valid Unicode text: sequences of Unicode scalar values. An `encoding` names the character set its values are held in, so every character is one that set can encode, and, with `ill_formed`, its values may be any sequence of that set's code units, valid or not. `code_points` narrows the characters further. How text compares, under a collation, is not part of its type, since how a source compares text changes no value. */
export interface TextType extends Base {
  kind: "text";
  /** The character set the values are held in, by its IANA name, pinned to one mapping table (spec/README.md): every character is one it encodes. A Unicode encoding (UTF-8, UTF-16, UTF-32, CESU-8, GB18030) encodes every character, so it is written only beside `ill_formed`, or where a size counts its code units or bytes. */
  encoding?: Charset;
  /** Values may be any sequence of the encoding's code units, not only valid text: bytes for UTF-8 or a single-byte set, 16-bit units for UTF-16 (a lone surrogate), 32-bit units up to 0x10FFFF for UTF-32. Written only beside `encoding`. */
  ill_formed?: true;
  /** Inclusive ranges of code points every character falls in, beside what `encoding` allows: `[[1, 1114111]]` holds no NUL, `[[0, 65535]]` nothing past the Basic Multilingual Plane. Sorted, disjoint and never adjacent. In ill-formed text, a lone surrogate or a 32-bit unit is held to them by its value. */
  code_points?: [number, number][];
  size?: TextSize;
  /** An I-Regexp (RFC 9485) every value matches whole: `[^@]+@[^@]+`. Matched against ill-formed text as decoded, each ill-formed sequence read as U+FFFD. */
  pattern?: string;
  /** What the text is, as a media type (`text/html`): JSON Schema's `contentMediaType`. */
  media_type?: string;
  /** What the text is, by JSON Schema's format name (`email`, `uri`, `hostname`, `ipv4`) or a source's own. It narrows the values, and a consumer that does not know a format ignores it. A format the format has a kind for is written as that kind: `date-time` is an `instant`, `uuid` a `uuid` and `binary` `bytes`. */
  format?: string;
}

/** A text's length: in code points where `unit` is absent, in the encoding's code units (UTF-16 units, as a JavaScript string's `length` counts), or in bytes in the encoding. A unit other than code points is written only where the text names its `encoding`. */
export interface TextSize extends Size {
  unit?: "code_unit" | "byte";
}

/** A character set, by its IANA name, matched without regard to case: the Unicode encodings; US-ASCII and ISO 8859, pinned to Unicode's mapping tables; the Windows code pages, pinned to the WHATWG Encoding Standard's indexes; and the sets databases and languages still store. */
export type Charset =
  | "UTF-8"
  | "UTF-16"
  | "UTF-32"
  | "CESU-8"
  | "GB18030"
  | "US-ASCII"
  | "ISO-8859-1"
  | "ISO-8859-2"
  | "ISO-8859-3"
  | "ISO-8859-4"
  | "ISO-8859-5"
  | "ISO-8859-6"
  | "ISO-8859-7"
  | "ISO-8859-8"
  | "ISO-8859-9"
  | "ISO-8859-10"
  | "ISO-8859-13"
  | "ISO-8859-14"
  | "ISO-8859-15"
  | "ISO-8859-16"
  | "windows-874"
  | "windows-1250"
  | "windows-1251"
  | "windows-1252"
  | "windows-1253"
  | "windows-1254"
  | "windows-1255"
  | "windows-1256"
  | "windows-1257"
  | "windows-1258"
  | "IBM437"
  | "IBM850"
  | "IBM852"
  | "IBM866"
  | "KOI8-R"
  | "KOI8-U"
  | "macintosh"
  | "TIS-620"
  | "DEC-MCS"
  | "hp-roman8"
  | "SEN_850200_B"
  | "Shift_JIS"
  | "Windows-31J"
  | "EUC-JP"
  | "ISO-2022-JP"
  | "EUC-KR"
  | "GB2312"
  | "GBK"
  | "Big5"
  | "Big5-HKSCS";

/** Bytes: any sequence of octets, of the size it states. Its canonical text is base64 (RFC 4648, padded). */
export interface BytesType extends Base, Sized {
  kind: "bytes";
  /** What the bytes are, as a media type (`image/png`): JSON Schema's `contentMediaType`. */
  media_type?: string;
}

/** A 128-bit identifier, in RFC 9562's text form. It holds any 128 bits, the nil UUID and bits no RFC version describes among them, unless `versions` says otherwise. */
export interface UuidType extends Base {
  kind: "uuid";
  /** Every value is of RFC 9562's variant, with one of these versions: `[7]`, or all eight for any RFC UUID. Written only where the source guarantees it. */
  versions?: number[];
}

/** One of a fixed list of values of `type`, in that type's canonical text: an enum's labels, a `CHECK (x IN (…))`, or a constant a statement writes. An enum of one value is a literal: `select 'user' as kind` is `{ kind: "enum", type: { kind: "text" }, values: ["user"] }`. The list's order is the source's, and how its values sort is not part of the type. */
export interface EnumType extends Base {
  kind: "enum";
  /** A kind whose values the format writes as text: a boolean (`"true"`), a number, text, bytes, a UUID, a date, a time or a duration. Never itself an enum. */
  type: Type;
  values: string[];
}

/** A calendar date. */
export interface DateType extends Base, Bounded {
  kind: "date";
  /** Also holds infinity and -infinity, which sort after and before every other value. Bounds constrain only the finite values. */
  infinity?: true;
}

/** A time of day. Its `max` may be `24:00:00`, which some languages' time types refuse. */
export interface TimeType extends Base, Bounded {
  kind: "time";
  tick: string;
  /** Each value keeps a UTC offset beside the time. */
  offset?: true;
}

/** A date and a time of day as a wall clock reads them, with no zone: no moment until a zone is given. */
export interface DateTimeType extends Base, Bounded {
  kind: "datetime";
  tick: string;
  /** Also holds infinity and -infinity, which sort after and before every other value. Bounds constrain only the finite values. */
  infinity?: true;
}

/** A point on the timeline and nothing else, as Temporal's `Instant`: written in UTC (`2026-11-14T18:14:00Z`). A source that prints one with an offset adds the offset as it prints. Bounds are in UTC. */
export interface InstantType extends Base, Bounded {
  kind: "instant";
  tick: string;
  /** Also holds infinity and -infinity, which sort after and before every other value. Bounds constrain only the finite values. */
  infinity?: true;
}

/** A point on the timeline and the time zone it was written in, a region or a UTC offset, as Temporal's `ZonedDateTime`: written as RFC 9557 does, `2026-11-14T10:14:00-08:00[America/Los_Angeles]`, or `2026-11-14T10:14:00-08:00[-08:00]` for an offset. Bounds compare the moments. */
export interface ZonedDateTimeType extends Base, Bounded {
  kind: "zoned_datetime";
  tick: string;
  /** Every value's zone is a UTC offset, never a region, as Java's `OffsetDateTime`. */
  offset_only?: true;
  /** Also holds infinity and -infinity, which sort after and before every other value. Bounds constrain only the finite values. */
  infinity?: true;
}

/** A length of time, in the parts it has (months, days and time), each signed on its own. A month and a day have no fixed length (a month is 28 to 31 days, and a day 23 to 25 hours across a clock change), so a duration that counts them keeps them apart from its time. ISO 8601's intervals, a start and an end, are a `range` here. */
export interface DurationType extends Base {
  kind: "duration";
  /** The parts its values have, each at the largest magnitude it reaches, of either sign, as one ISO 8601 duration: months in `M`, days in `D`, then time in `H`, `M` and `S`, its minutes and seconds below 60. A part the type lacks is left out, and a part it has is never 0. A two's complement part reaches one unit further below zero than above it, so it is written at the negative side's magnitude: 32-bit months are `2147483648M`. */
  max: string;
  /** The time part's smallest step, written exactly when `max` has a time part. */
  tick?: string;
  /** Also holds infinity and -infinity. */
  infinity?: true;
}

/** A sequence of elements, in order, each in the one `element` slot. An array is flat: a two-dimensional array is an array of arrays, each level of its own. Where its index starts is not part of it. Its size counts its elements, so a vector of 3 floats is `size: { exact: 3 }`, and a bit string an array of booleans of its length. */
export interface ArrayType extends Base, Sized {
  kind: "array";
  element: Slot;
  /** No two elements are equal, by the source's own equality, and so no two are the same value: a set, as JSON Schema's `uniqueItems` says. */
  distinct?: true;
}

/** A sequence of values, each of its own type, with no names: `[1, "a"]`. `elements` come first and are always there, `optional_elements` follow, each present only where the one before it is, and `rest` takes any number after them. JSON Schema writes the same with `prefixItems`, `minItems` and `items`. */
export interface TupleType extends Base {
  kind: "tuple";
  /** The elements every value has, in order. */
  elements: Slot[];
  /** Elements that may be missing from the end, in order: a value with the second of them has the first. */
  optional_elements?: Slot[];
  /** Any number of further elements, each in this slot. */
  rest?: Slot;
}

/** Named fields, in order: a row, a composite type, an object. A struct holds exactly its fields, unless `extra` lets it hold other keys too. A source's sugar over simpler values is a struct carrying its `label`, as a point is `{ x, y }`. */
export interface StructType extends Base {
  kind: "struct";
  fields: Field[];
  /** Keys beyond its fields, any number, each holding a value in this slot, as JSON Schema's `additionalProperties`: it says nothing of the named fields. A key is text, and none is a field's name. */
  extra?: Slot;
}

/** Keys to values, each key once and in no order: a dictionary. A map whose keys may repeat is an array of `{ key, value }` structs. Its size counts its keys, as JSON Schema's `minProperties` and `maxProperties` do. */
export interface MapType extends Base, Sized {
  kind: "map";
  /** Inside JSON content, a key is text and never null. */
  key: Slot;
  value: Slot;
}

/** Rows of columns: what a statement returns, and a table nested in a value, such as SQL's `MULTISET`. A row of a table is a struct of its columns; a language may hold a table as an array of objects, a list of tuples, or columns. */
export interface TableType extends Base {
  kind: "table";
  /** How many rows: exactly one, at most one, any number, or none (a write that returns nothing). */
  cardinality: "one" | "at_most_one" | "many" | "none";
  /** In order; empty where there are no rows. */
  columns: Field[];
  /** Each set of columns, by name, no two rows agree on all of: no two equal under the source's own equality, a null equal to another null, and so no two the same values. None inside another: `[["id"], ["email"]]`. A column of a unique index counts only where its nulls cannot repeat. Only where the cardinality is `many`. */
  unique?: string[][];
  /** The columns the rows are sorted on, in order, by the source's own sort, as far as the sort names columns of the table: `order by at desc, lower(name), id` gives only `at`, since its next key is no column, and `id` then sorts only rows that tie. A key under another collation than its column's ends the list. Rows that tie on all of them come in any order. Only where the cardinality is `many`. */
  order?: SortKey[];
}

/** A column a table's rows are sorted on. */
export interface SortKey {
  /** The column's name. */
  column: string;
  /** Sorted from the highest value down. */
  desc?: true;
  /** Where nulls sort. */
  nulls: "first" | "last";
}

/**
 * A JSON value.
 *   - `content`: what a statement built, described with the same types, each leaf typed by the value it was written from, so a 64-bit integer may pass 2^53.
 *   - `schema`: a JSON Schema every value satisfies, whose `$schema` names the draft it is read by.
 *   - Neither: any JSON, whose numbers may have any length.
 */
export interface JsonType extends Base {
  kind: "json";
  /** Where it may be null, the document may be JSON null. */
  content?: Slot;
  schema?: { $schema: string; [keyword: string]: unknown; };
}

/** One of several types: each value is one of them. A union whose values say which member they are is a `tagged_union`. */
export interface UnionType extends Base {
  kind: "union";
  members: Type[];
}

/** Values of every member at once: JSON Schema's `allOf`, TypeScript's `&`. An intersection one struct can write, of structs whose fields agree or of a struct and a map with text keys, is written as that struct. */
export interface IntersectionType extends Base {
  kind: "intersection";
  members: Type[];
}

/** One of several alternatives, each named by the tag every value carries, in the order the type declares them, as a Rust enum. Unlike a union's members, two alternatives may hold one type, and a value says which alternative it is even where it holds null. */
export interface TaggedUnionType extends Base {
  kind: "tagged_union";
  alternatives: Alternative[];
}

/** An alternative of a tagged union: its tag, unique in the type, and the slot of the value it holds. */
export interface Alternative extends Slot {
  tag: string;
}

/** A range of `subtype`, with an empty range and unbounded ends: a start and an end. Over a discrete subtype, such as integers or dates, `[1,3]` and `[1,4)` are one range. An unbounded end lies beyond the subtype's infinities, so `[x,)` holds infinity and `[x,infinity)` does not. */
export interface RangeType extends Base {
  kind: "range";
  subtype: Type;
}

/** An ordered set of non-overlapping ranges. */
export interface MultirangeType extends Base {
  kind: "multirange";
  subtype: Type;
}

/** An OGC simple feature, planar or on a spheroid (`geodetic`). */
export interface GeometryType extends Base {
  kind: "geometry";
  model?: "planar" | "geodetic";
  srid?: number;
  subtype?: "point" | "linestring" | "polygon" | "multipoint" | "multilinestring" | "multipolygon" | "geometrycollection";
  coordinates?: "xy" | "xyz" | "xym" | "xyzm";
}

/** XML: a document with one root, valid against any `schemas` given, or a fragment, which may have several roots (`<a/><b/>`). An XML Schema validates documents only, so a fragment names none. */
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

/** The value that is always null, in a slot marked `nullable`: the field of a `json_build_object('a', null)`. */
export interface NullType {
  kind: "null";
}

/** A type the source knows and the format does not structure: an extension's own type, or one that holds a value of any type, each carrying its own. Its `label` is all the format says; what a value looks like is the driver's to decide. */
export interface OpaqueType extends Base {
  kind: "opaque";
  label: string;
}

/** Nothing is known about the values, but whether one may be null, which its slot says. */
export interface UnknownType {
  kind: "unknown";
}

/** What introspecting one statement returns. A tool may write keys of its own beside these, as `reylo analyze` writes the file a statement was read from. */
export interface StatementDescriptor {
  /** The version of the format the descriptor is written in, as the URL of that version's JSON Schema, which a validator or an editor reads. Versions are semantic, and until 1.0.0 a new minor version may change anything. */
  $schema: "https://reylo.dev/typedefs/0.1.1.json";
  dialect: Dialect;
  /** The statement as the database runs it, with its own placeholders (`$1` on Postgres, `?` on MySQL and SQLite), in the order `params` lists them. */
  sql: string;
  /** `select` for any query, a `VALUES` list and a set operation included; `other` for any statement that is none of the five: a `CALL`, whose procedure may write, an `EXPLAIN`, a `SHOW` or a SQLite `PRAGMA`, and DDL. Absent when the statement does not parse. */
  kind?: "select" | "insert" | "update" | "delete" | "merge" | "other";
  /** The prose written in comments before the statement: its summary, as an editor's hover shows it. */
  comment?: string;
  params: Param[];
  /** What the statement returns, a table: a row of columns per row, and its cardinality. A statement that returns one value is still a row of one column, which keeps the column's name. A row is an object keyed by its columns' names, which keeps only the last of two with one name, so a statement whose result repeats a name (`select a.id, b.id`) has a diagnostic at each later column of it, asking for another name with `as`. The name the database gives a column stands: Postgres calls `select 1`'s `?column?`. */
  result: TableType;
  /** What running the statement may do, an upper bound: a flag it lacks, or a table it does not name, is something it cannot do, so a statement whose tables are all `read` and that has no flag only reads. Absent, nothing is said. */
  capabilities?: Capabilities;
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

/** Where a type is used: a column, a parameter, a struct's field, an array's element, a tuple's element, a map's key or value, a tagged union's alternative, JSON content. Whether a value may be null is the slot's to say, never the type's, so one type reads the same at every use. A union's members, an intersection's, an enum's type and a range's subtype are bare types, since a value stands at none of them. */
export interface Slot {
  type: Type;
  /** The value may be null, whatever the type's values, bounds or label say. On a parameter, the statement accepts null; inside JSON content, the value may be JSON null. */
  nullable?: true;
}

/** A struct's field or a table's column: a key of an object, a column of a row. */
export interface Field extends Slot {
  name: string;
  /** The key may be absent, apart from whether its value may be null; `default` names the value it takes where it is absent. */
  optional?: true;
  /** The value an optional field takes where its key is absent, in its type's canonical text: only beside `optional`. */
  default?: string;
  /** What the field is, as its source documents it, such as the comment a database keeps on a column. */
  comment?: string;
  /** A short name for the field, as JSON Schema's `title`. */
  title?: string;
  /** Only values the source gives back hold the field, and values sent to it leave it out: JSON Schema's `readOnly`, such as an id the source assigns. Never beside `write_only`. */
  read_only?: true;
  /** Only values sent to the source hold the field, and values it gives back leave it out: JSON Schema's `writeOnly`, such as a password. Never beside `read_only`. */
  write_only?: true;
}

/** A statement's parameter. Where the statement writes it whole into a column, its type is that column's: `insert into t (price) values (:price)` into a decimal of scale 2 gives `price` that scale, though the database describes the parameter with none and would round 1.234. That is stricter than the server, never looser. */
export interface Param extends Slot {
  name: string;
  /** The caller may leave the parameter out. */
  optional?: true;
  /** The comment the database keeps on the column the parameter is written to whole. */
  comment?: string;
}

/** The database a document of descriptors describes, stated once at the document's top: every `label` inside is this dialect's. `name` is the database's, such as `postgres`, `mysql`, `sqlite`, `duckdb`, `oracle` or `sqlserver`, and `version` its server version. */
export interface Dialect {
  name: string;
  version: string;
}

/** What running a statement may do: the tables it touches, and four kinds of effect beyond their rows. */
export interface Capabilities {
  /** Each table the statement touches, qualified as a `label` is (`public.posts`), with the most it may do to its rows, through a view, a partition, a foreign key's action or a sequence's default too; `unknown` where it may touch tables the analysis cannot name, as through a function or a trigger whose body it does not read. */
  tables: { [table: string]: Access; } | "unknown";
  /** Creates, alters or drops what the database defines, or changes who may use it: `CREATE`, `ALTER`, `DROP`, `GRANT`. */
  ddl?: true;
  /** Begins or ends a transaction: `BEGIN`, `COMMIT`, `SAVEPOINT`, and MySQL's DDL, which commits. */
  transaction?: true;
  /** Leaves state on the connection after it: `SET`, a temporary table, a session's advisory lock, `PREPARE`, `LISTEN`, MySQL's `USE`, a SQLite `PRAGMA` or `ATTACH`. */
  session?: true;
  /** Reaches past the connection and the data: the server's settings (`ALTER SYSTEM`, `SET GLOBAL`), another session (`NOTIFY`, ending a backend), a file or the network. */
  external?: true;
}

/** The most a statement may do to a table's rows, each covering the ones before it: `read`; `readlock`, which reads them and locks them until its transaction ends (`FOR UPDATE`, `LOCK TABLE`); and `write`, which inserts, updates or deletes them. */
export type Access = "read" | "readlock" | "write";
