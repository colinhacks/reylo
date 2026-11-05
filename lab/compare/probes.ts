// The queries every tool is asked to type, and the right answer to each. A row of the comparison is graded from these answers alone (grade.ts).
export type Db = "postgres" | "mysql" | "sqlite";
export const DBS: Db[] = ["postgres", "mysql", "sqlite"];
export type Kind = "num" | "text" | "bool" | "time" | "json" | "any";
export type Card = "one" | "maybeOne" | "many" | "none";

// `anyNullOn`: databases where either answer is right. MySQL's string functions return NULL when the result would exceed max_allowed_packet, whatever their arguments, so its server calls them nullable and a tool may follow it or not.
export interface WantColumn { name: string; nullable?: boolean; shape?: string[]; anyNullOn?: Db[] }
// `value`: the TypeScript type of the value a caller passes, for a tool that checks the value at each call instead of typing the parameter.
// `anyNullOn`: databases where either answer is right, as SQLite's INTEGER PRIMARY KEY, which assigns a rowid for a null.
export interface WantParam { kinds?: Kind[]; nullable?: boolean; shape?: string[]; value?: string; anyNullOn?: Db[] }
export interface Probe {
  id: string;
  // Placeholders are written $1, $2, … in order of appearance; each tool's adapter rewrites them to its own form.
  sql: string | Partial<Record<Db, string>>;
  columns?: WantColumn[];
  params?: WantParam[];
  cardinality?: Card;
}
export const sqlFor = (p: Probe, db: Db): string | undefined => (typeof p.sql === "string" ? p.sql : p.sql[db]);

const NUM: Kind[] = ["num"];
const TEXT: Kind[] = ["text"];
const nn = (name: string): WantColumn => ({ name, nullable: false });
const nu = (name: string): WantColumn => ({ name, nullable: true });
const v = (nullable: boolean): WantColumn[] => [{ name: "v", nullable }];
const NO_FULL_JOIN = (sql: string) => ({ postgres: sql, sqlite: sql });
const concat = (arg: string) => ({ postgres: `select ${arg} || '!' as v from users`, sqlite: `select ${arg} || '!' as v from users`, mysql: `select concat(${arg}, '!') as v from users` });

export const OUTER_JOINS: Probe[] = [
  { id: "left", sql: "select u.name, p.title from users u left join posts p on p.author_id = u.id", columns: [nn("name"), nu("title")] },
  { id: "left-sorted", sql: "select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title", columns: [nn("name"), nu("title")] },
  { id: "left-where", sql: "select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1", columns: [nn("name"), nu("title")], params: [{ value: "number" }] },
  { id: "left-subquery", sql: "select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id", columns: [nn("name"), nu("title")] },
  { id: "right", sql: "select u.name, p.title from posts p right join users u on u.id = p.author_id", columns: [nn("name"), nu("title")] },
  { id: "full", sql: NO_FULL_JOIN("select u.name, p.title from users u full join posts p on p.author_id = u.id"), columns: [nu("name"), nu("title")] },
];

export const EXPRESSIONS: Probe[] = [
  { id: "count", sql: "select count(*) as v from posts", columns: v(false) },
  { id: "max", sql: "select max(id) as v from posts", columns: v(true) },
  { id: "sum", sql: "select sum(score) as v from posts", columns: v(true) },
  { id: "coalesce", sql: "select coalesce(nickname, 'x') as v from users", columns: v(false) },
  { id: "concat", sql: concat("name"), columns: [{ name: "v", nullable: false, anyNullOn: ["mysql"] }] },
  { id: "concat-nullable", sql: concat("nickname"), columns: v(true) },
  { id: "upper", sql: "select upper(name) as v from users", columns: [{ name: "v", nullable: false, anyNullOn: ["mysql"] }] },
  { id: "upper-nullable", sql: "select upper(nickname) as v from users", columns: v(true) },
  { id: "add", sql: "select id + 1 as v from posts", columns: v(false) },
  { id: "add-nullable", sql: "select score + 1 as v from posts", columns: v(true) },
  { id: "nullif", sql: "select nullif(name, 'x') as v from users", columns: v(true) },
  { id: "case-no-else", sql: "select case when id > 0 then name end as v from users", columns: v(true) },
  { id: "is-null", sql: "select nickname is null as v from users", columns: v(false) },
  { id: "subquery-count", sql: "select (select count(*) from posts) as v", columns: v(false) },
  { id: "exists", sql: "select exists (select 1 from posts) as v", columns: v(false) },
  { id: "row-number", sql: "select row_number() over (order by id) as v from users", columns: v(false) },
  { id: "lag", sql: "select lag(id) over (order by id) as v from users", columns: v(true) },
  { id: "union", sql: "select id as v from users union all select score from posts", columns: v(true) },
  { id: "grouped-max", sql: "select max(id) as v from posts group by author_id", columns: v(false) },
];

export const PARAMETER_TYPES: Probe[] = [
  { id: "eq-int", sql: "select name from users where id = $1", params: [{ kinds: NUM }] },
  { id: "eq-text", sql: "select id from users where name = $1", params: [{ kinds: TEXT }] },
  { id: "insert", sql: "insert into posts (id, author_id, title, score) values ($1, $2, $3, $4)", params: [{ kinds: NUM }, { kinds: NUM }, { kinds: TEXT }, { kinds: NUM }] },
  { id: "limit", sql: "select id from users limit $1", params: [{ kinds: NUM }] },
  { id: "like", sql: "select id from users where name like $1", params: [{ kinds: TEXT }] },
  { id: "uuid", sql: { postgres: "select id from users where external_id = $1" }, params: [{ kinds: TEXT }] },
  { id: "timestamp", sql: { postgres: "select id from posts where created_at > $1" }, params: [{ kinds: ["time", "text"] }] },
];

export const PARAMETER_NULLABILITY: Probe[] = [
  { id: "insert", sql: "insert into users (id, email, name, nickname) values ($1, $2, $3, $4)", params: [{ nullable: false, anyNullOn: ["sqlite"], value: "number" }, { nullable: false, value: "string" }, { nullable: false, value: "string" }, { nullable: true, value: "string" }] },
  { id: "update", sql: "update users set nickname = $1 where id = $2", params: [{ nullable: true, value: "string" }, { nullable: false, value: "number" }] },
  { id: "eq-key", sql: "select name from users where id = $1", params: [{ nullable: false, value: "number" }] },
  { id: "eq-nullable", sql: "select id from users where nickname = $1", params: [{ nullable: false, value: "string" }] },
];

export const CARDINALITY: Probe[] = [
  { id: "primary-key", sql: "select id, name from users where id = $1", cardinality: "maybeOne", params: [{ value: "number" }] },
  { id: "unique", sql: "select id from users where email = $1", cardinality: "maybeOne", params: [{ value: "string" }] },
  { id: "count", sql: "select count(*) as v from users", cardinality: "one" },
  { id: "insert-returning", sql: { postgres: "insert into users (id, email, name) values ($1, $2, $3) returning id", sqlite: "insert into users (id, email, name) values ($1, $2, $3) returning id" }, cardinality: "one", params: [{ value: "number" }, { value: "string" }, { value: "string" }] },
  { id: "all", sql: "select id from users", cardinality: "many" },
  { id: "limit-1", sql: "select id from users limit 1", cardinality: "maybeOne" },
  { id: "update-returning", sql: { postgres: "update users set name = $1 where id = $2 returning id", sqlite: "update users set name = $1 where id = $2 returning id" }, cardinality: "maybeOne", params: [{ value: "string" }, { value: "number" }] },
  { id: "join-on-keys", sql: "select p.title from posts p join users u on u.id = p.author_id where p.id = $1", cardinality: "maybeOne", params: [{ value: "number" }] },
  { id: "non-key", sql: "select id from posts where author_id = $1", cardinality: "many", params: [{ value: "number" }] },
  { id: "delete", sql: "delete from users where id = $1", cardinality: "none", params: [{ value: "number" }] },
];

export const JSON_SHAPES: Probe[] = [
  { id: "object", sql: { postgres: "select json_build_object('id', id, 'name', name) as v from users", mysql: "select json_object('id', id, 'name', name) as v from users", sqlite: "select json_object('id', id, 'name', name) as v from users" }, columns: [{ name: "v", shape: ["id", "name"] }] },
  { id: "aggregate", sql: { postgres: "select json_agg(json_build_object('id', id, 'title', title)) as v from posts", mysql: "select json_arrayagg(json_object('id', id, 'title', title)) as v from posts", sqlite: "select json_group_array(json_object('id', id, 'title', title)) as v from posts" }, columns: [{ name: "v", shape: ["id", "title"] }] },
];

export const JSON_SCHEMA: Probe[] = [
  { id: "column", sql: { postgres: "select payload from events" }, columns: [{ name: "payload", shape: ["x"] }] },
  { id: "parameter", sql: { postgres: "insert into events (id, payload) values ($1, $2)" }, params: [{ value: "number" }, { shape: ["x"], value: "{ x: number }" }] },
];

// What only a database knows: its functions' result types, and each operator's result type.
export const FROM_THE_DATABASE: Probe[] = [
  { id: "function", sql: { postgres: "select add_one(id) as v from users" }, columns: [{ name: "v" }], params: [] },
  { id: "operator", sql: { postgres: "select id::numeric / 2 as v from users" }, columns: [{ name: "v" }], params: [] },
  { id: "builtin", sql: { postgres: "select array_agg(id) as v from users" }, columns: [{ name: "v" }], params: [] },
];
export const FROM_THE_DATABASE_KINDS: Record<string, Kind[]> = { function: ["num"], operator: ["num", "text"], builtin: ["json", "num", "text"] };

export const SMOKE: Probe = { id: "smoke", sql: "select id, name from users", columns: [nn("id"), nn("name")] };

export const ALL_PROBES: Probe[] = [...OUTER_JOINS, ...EXPRESSIONS, ...PARAMETER_TYPES, ...PARAMETER_NULLABILITY, ...CARDINALITY, ...JSON_SHAPES, ...JSON_SCHEMA, ...FROM_THE_DATABASE, SMOKE];
