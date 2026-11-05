# Results

Written by `./run.sh` on 2026-11-05. Each mark comes from what the tool answered; [probes.ts](probes.ts) holds the queries and the right answers, and [grade.ts](grade.ts) the rules.

| | Reylo | sqlc | PgTyped | TypedSQL | sqlx-ts | SafeQL | sqlx | TypeSQL | Postgres Typed SQL |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Version | 0.7.0 | v1.31.1 | 2.4.3 | 7.10.1 | 0.49.0 | 5.4.2 | 0.9.0 | 0.24.0 | 0.1.0-beta.22 |
| No codegen | yes | no | no | no | no | partly | yes | no | partly |
| Live database | yes (3/3) | partly (3/3) | yes (3/3) | yes (3/3) | partly (1/3) | partly (2/3) | yes (3/3) | yes (3/3) | no (3/3) |
| Nullable left joins | yes (17/17) | partly (14/17) | no (0/6) | partly (12/17) | no (0/17) | yes (6/6) | partly (12/17) | partly (11/17) | yes (6/6) |
| Nullable expressions | yes (57/57) | partly (37/57) | no (9/19) | partly (44/57) | partly (30/57) | partly (15/19) | partly (44/57) | partly (49/57) | yes (19/19) |
| Parameter types | yes (17/17) | yes (17/17) | yes (7/7) | partly (12/17) | partly (16/17) | partly (5/7) | partly (7/17) | partly (16/17) | yes (7/7) |
| Nullable parameters | yes (12/12) | partly (9/12) | no (0/4) | no (4/12) | partly (9/12) | no (0/4) | no (0/12) | yes (12/12) | no (2/4) |
| Cardinality | yes (28/28) | no (0/28) | no (0/10) | no (0/28) | no (0/28) | no (0/10) | no (0/28) | partly (16/28) | partly (9/10) |
| Nested JSON | yes (6/6) | no (0/6) | no (0/2) | no (0/6) | partly (5/6) | yes (2/2) | no (0/6) | partly (2/6) | yes (2/2) |
| JSON Schema types | yes (2/2) | no (0/2) | no (0/2) | no (0/2) | no (0/2) | no (0/2) | no (0/2) | no (0/2) | no (0/2) |
| Postgres | yes | yes | yes | yes | yes | yes | yes | yes | yes |
| MySQL | yes | yes | no | yes | yes | no | yes | yes | no |
| SQLite | yes | yes | no | yes | yes | no | yes | yes | no |
| DuckDB | yes | no | no | no | no | no | no | no | no |

## Reylo 0.7.0

**No codegen**: yes. Nothing to commit after a run.

## sqlc v1.31.1

**No codegen**: no. A run leaves new files to commit: `db/db.go`, `db/models.go`, `db/query.sql.go`.

**Live database**: partly, 3 of 3. By default it reads your schema files and never connects. A `database:` setting asks Postgres for types, but not for nullability.

**Nullable left joins**: partly, 14 of 17

- postgres: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: `title` is typed `string`, but it can be null
- mysql: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: `title` is typed `string`, but it can be null

**Nullable expressions**: partly, 37 of 57

- postgres: `select sum(score) as v from posts`: `v` is typed `int64`, but it can be null
- postgres: `select name || '!' as v from users`: `v` is typed `interface{}`, but it is never null
- postgres: `select upper(name) as v from users`: `v` is typed `interface{}`, but it is never null
- postgres: `select score + 1 as v from posts`: `v` is typed `int32`, but it can be null
- postgres: `select nullif(name, 'x') as v from users`: `v` is typed `bool`, but it can be null
- postgres: `select nickname is null as v from users`: `v` is typed `interface{}`, but it is never null
- postgres: `select id as v from users union all select score from posts`: `v` is typed `int32`, but it can be null
- postgres: `select max(id) as v from posts group by author_id`: `v` is typed `interface{}`, but it is never null
- mysql: `select concat(nickname, '!') as v from users`: `v` is typed `string`, but it can be null
- mysql: `select upper(nickname) as v from users`: `v` is typed `string`, but it can be null
- mysql: `select score + 1 as v from posts`: `v` is typed `int32`, but it can be null
- mysql: `select row_number() over (order by id) as v from users`: `v` is typed `interface{}`, but it is never null
- mysql: `select id as v from users union all select score from posts`: `v` is typed `int32`, but it can be null
- mysql: `select max(id) as v from posts group by author_id`: `v` is typed `interface{}`, but it is never null
- sqlite: `select name || '!' as v from users`: `v` is typed `interface{}`, but it is never null
- sqlite: `select upper(nickname) as v from users`: `v` is typed `string`, but it can be null
- sqlite: `select score + 1 as v from posts`: `v` is typed `int64`, but it can be null
- sqlite: `select row_number() over (order by id) as v from users`: `v` is typed `interface{}`, but it is never null
- sqlite: `select id as v from users union all select score from posts`: `v` is typed `int64`, but it can be null
- sqlite: `select max(id) as v from posts group by author_id`: `v` is typed `interface{}`, but it is never null

**Nullable parameters**: partly, 9 of 12

- postgres: `select id from users where nickname = $1`: parameter 1 is typed `sql.NullString`, but it takes no null
- mysql: `select id from users where nickname = $1`: parameter 1 is typed `sql.NullString`, but it takes no null
- sqlite: `select id from users where nickname = $1`: parameter 1 is typed `sql.NullString`, but it takes no null

**Cardinality**: no, 0 of 28. Declared by hand on each query: `:one`, `:many` or `:exec`. The annotation decides the method's return type, and sqlc accepts any of them.

- postgres: `select id, name from users where id = $1`: it does not infer one
- postgres: `select id from users where email = $1`: it does not infer one
- postgres: `select count(*) as v from users`: it does not infer one
- postgres: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- postgres: `select id from users`: it does not infer one
- postgres: `select id from users limit 1`: it does not infer one
- postgres: `update users set name = $1 where id = $2 returning id`: it does not infer one
- postgres: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- postgres: `select id from posts where author_id = $1`: it does not infer one
- postgres: `delete from users where id = $1`: it does not infer one
- mysql: `select id, name from users where id = $1`: it does not infer one
- mysql: `select id from users where email = $1`: it does not infer one
- mysql: `select count(*) as v from users`: it does not infer one
- mysql: `select id from users`: it does not infer one
- mysql: `select id from users limit 1`: it does not infer one
- mysql: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- mysql: `select id from posts where author_id = $1`: it does not infer one
- mysql: `delete from users where id = $1`: it does not infer one
- sqlite: `select id, name from users where id = $1`: it does not infer one
- sqlite: `select id from users where email = $1`: it does not infer one
- sqlite: `select count(*) as v from users`: it does not infer one
- sqlite: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- sqlite: `select id from users`: it does not infer one
- sqlite: `select id from users limit 1`: it does not infer one
- sqlite: `update users set name = $1 where id = $2 returning id`: it does not infer one
- sqlite: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- sqlite: `select id from posts where author_id = $1`: it does not infer one
- sqlite: `delete from users where id = $1`: it does not infer one

**Nested JSON**: no, 0 of 6

- postgres: `select json_build_object('id', id, 'name', name) as v from users`: `v` is typed `json.RawMessage`, without its fields
- postgres: `select json_agg(json_build_object('id', id, 'title', title)) as v from posts`: `v` is typed `json.RawMessage`, without its fields
- mysql: `select json_object('id', id, 'name', name) as v from users`: `v` is typed `json.RawMessage`, without its fields
- mysql: `select json_arrayagg(json_object('id', id, 'title', title)) as v from posts`: `v` is typed `json.RawMessage`, without its fields
- sqlite: `select json_object('id', id, 'name', name) as v from users`: `v` is typed `interface{}`, without its fields
- sqlite: `select json_group_array(json_object('id', id, 'title', title)) as v from posts`: `v` is typed `interface{}`, without its fields

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `json.RawMessage`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: parameter 2 is typed `json.RawMessage`, without its fields

**DuckDB**: no. Not in v1.31.1, the latest release. A DuckDB engine is on its unreleased main branch.

## PgTyped 2.4.3

**No codegen**: no. A run leaves new files to commit: `src/q.queries.ts`.

**Nullable left joins**: no, 0 of 6

- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1`: `title` is typed `string`, but it can be null
- postgres: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u full join posts p on p.author_id = u.id`: `name` is typed `string`, but it can be null; `title` is typed `string`, but it can be null

**Nullable expressions**: no, 9 of 19. It gives every expression the same answer.

- postgres: `select count(*) as v from posts`: `v` is typed `string | null`, but it is never null
- postgres: `select coalesce(nickname, 'x') as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select name || '!' as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select upper(name) as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select id + 1 as v from posts`: `v` is typed `number | null`, but it is never null
- postgres: `select nickname is null as v from users`: `v` is typed `boolean | null`, but it is never null
- postgres: `select (select count(*) from posts) as v`: `v` is typed `string | null`, but it is never null
- postgres: `select exists (select 1 from posts) as v`: `v` is typed `boolean | null`, but it is never null
- postgres: `select row_number() over (order by id) as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select max(id) as v from posts group by author_id`: `v` is typed `number | null`, but it is never null

**Nullable parameters**: no, 0 of 4. It gives every parameter the same answer.

- postgres: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 1 is typed `optional number | null | void`, but it takes no null; parameter 2 is typed `optional string | null | void`, but it takes no null; parameter 3 is typed `optional string | null | void`, but it takes no null
- postgres: `update users set nickname = $1 where id = $2`: parameter 2 is typed `optional number | null | void`, but it takes no null
- postgres: `select name from users where id = $1`: parameter 1 is typed `optional number | null | void`, but it takes no null
- postgres: `select id from users where nickname = $1`: parameter 1 is typed `optional string | null | void`, but it takes no null

**Cardinality**: no, 0 of 10. A query's `.run()` always returns an array, whatever the statement.

- postgres: `select id, name from users where id = $1`: it does not infer one
- postgres: `select id from users where email = $1`: it does not infer one
- postgres: `select count(*) as v from users`: it does not infer one
- postgres: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- postgres: `select id from users`: it does not infer one
- postgres: `select id from users limit 1`: it does not infer one
- postgres: `update users set name = $1 where id = $2 returning id`: it does not infer one
- postgres: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- postgres: `select id from posts where author_id = $1`: it does not infer one
- postgres: `delete from users where id = $1`: it does not infer one

**Nested JSON**: no, 0 of 2

- postgres: `select json_build_object('id', id, 'name', name) as v from users`: `v` is typed `Json | null`, without its fields
- postgres: `select json_agg(json_build_object('id', id, 'title', title)) as v from posts`: `v` is typed `Json | null`, without its fields

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `Json`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: parameter 2 is typed `optional Json | null | void`, without its fields

**MySQL**: no. It is not available for MySQL.

**SQLite**: no. It is not available for SQLite.

**DuckDB**: no. PgTyped is Postgres only.

## TypedSQL 7.10.1

**No codegen**: no. A run leaves new files to commit: `generated/prisma/browser.ts`, `generated/prisma/client.ts`, `generated/prisma/commonInputTypes.ts`.

**Nullable left joins**: partly, 12 of 17

- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id`: `name` is typed `string | null`, but it is never null; `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1`: `title` is typed `string`, but it can be null
- postgres: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: `name` is typed `string | null`, but it is never null; `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `name` is typed `string | null`, but it is never null; `title` is typed `string`, but it can be null

**Nullable expressions**: partly, 44 of 57

- postgres: `select count(*) as v from posts`: `v` is typed `bigint | null`, but it is never null
- postgres: `select coalesce(nickname, 'x') as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select name || '!' as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select upper(name) as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select id + 1 as v from posts`: `v` is typed `number | null`, but it is never null
- postgres: `select nickname is null as v from users`: `v` is typed `boolean | null`, but it is never null
- postgres: `select (select count(*) from posts) as v`: `v` is typed `bigint | null`, but it is never null
- postgres: `select exists (select 1 from posts) as v`: `v` is typed `boolean | null`, but it is never null
- postgres: `select row_number() over (order by id) as v from users`: `v` is typed `bigint | null`, but it is never null
- postgres: `select max(id) as v from posts group by author_id`: `v` is typed `number | null`, but it is never null
- mysql: `select (select count(*) from posts) as v`: `v` is typed `bigint | null`, but it is never null
- mysql: `select max(id) as v from posts group by author_id`: `v` is typed `number | null`, but it is never null
- sqlite: `select upper(name) as v from users`: `v` is typed `string | null`, but it is never null

**Parameter types**: partly, 12 of 17

- sqlite: `select name from users where id = $1`: parameter 1 is typed `unknown`, but it takes a number
- sqlite: `select id from users where name = $1`: parameter 1 is typed `unknown`, but it takes text
- sqlite: `insert into posts (id, author_id, title, score) values ($1, $2, $3, $4)`: parameter 1 is typed `unknown`, but it takes a number; parameter 2 is typed `unknown`, but it takes a number; parameter 3 is typed `unknown`, but it takes text; parameter 4 is typed `unknown`, but it takes a number
- sqlite: `select id from users limit $1`: parameter 1 is typed `unknown`, but it takes a number
- sqlite: `select id from users where name like $1`: parameter 1 is typed `unknown`, but it takes text

**Nullable parameters**: no, 4 of 12. It gives every parameter the same answer.

- postgres: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 4 is typed `string`, but it takes null
- postgres: `update users set nickname = $1 where id = $2`: parameter 1 is typed `string`, but it takes null
- mysql: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 4 is typed `string`, but it takes null
- mysql: `update users set nickname = $1 where id = $2`: parameter 1 is typed `string`, but it takes null
- sqlite: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 2 is typed `unknown`, but it takes no null; parameter 3 is typed `unknown`, but it takes no null
- sqlite: `update users set nickname = $1 where id = $2`: parameter 2 is typed `unknown`, but it takes no null
- sqlite: `select name from users where id = $1`: parameter 1 is typed `unknown`, but it takes no null
- sqlite: `select id from users where nickname = $1`: parameter 1 is typed `unknown`, but it takes no null

**Cardinality**: no, 0 of 28. Every query returns an array, whatever the statement.

- postgres: `select id, name from users where id = $1`: it does not infer one
- postgres: `select id from users where email = $1`: it does not infer one
- postgres: `select count(*) as v from users`: it does not infer one
- postgres: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- postgres: `select id from users`: it does not infer one
- postgres: `select id from users limit 1`: it does not infer one
- postgres: `update users set name = $1 where id = $2 returning id`: it does not infer one
- postgres: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- postgres: `select id from posts where author_id = $1`: it does not infer one
- postgres: `delete from users where id = $1`: it does not infer one
- mysql: `select id, name from users where id = $1`: it does not infer one
- mysql: `select id from users where email = $1`: it does not infer one
- mysql: `select count(*) as v from users`: it does not infer one
- mysql: `select id from users`: it does not infer one
- mysql: `select id from users limit 1`: it does not infer one
- mysql: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- mysql: `select id from posts where author_id = $1`: it does not infer one
- mysql: `delete from users where id = $1`: it does not infer one
- sqlite: `select id, name from users where id = $1`: it does not infer one
- sqlite: `select id from users where email = $1`: it does not infer one
- sqlite: `select count(*) as v from users`: it does not infer one
- sqlite: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- sqlite: `select id from users`: it does not infer one
- sqlite: `select id from users limit 1`: it does not infer one
- sqlite: `update users set name = $1 where id = $2 returning id`: it does not infer one
- sqlite: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- sqlite: `select id from posts where author_id = $1`: it does not infer one
- sqlite: `delete from users where id = $1`: it does not infer one

**Nested JSON**: no, 0 of 6

- postgres: `select json_build_object('id', id, 'name', name) as v from users`: `v` is typed `$runtime.JsonValue | null`, without its fields
- postgres: `select json_agg(json_build_object('id', id, 'title', title)) as v from posts`: `v` is typed `$runtime.JsonValue | null`, without its fields
- mysql: `select json_object('id', id, 'name', name) as v from users`: `v` is typed `$runtime.JsonValue | null`, without its fields
- mysql: `select json_arrayagg(json_object('id', id, 'title', title)) as v from posts`: `v` is typed `$runtime.JsonValue | null`, without its fields
- sqlite: `select json_object('id', id, 'name', name) as v from users`: `v` is typed `string | null`, without its fields
- sqlite: `select json_group_array(json_object('id', id, 'title', title)) as v from posts`: `v` is typed `string | null`, without its fields

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `$runtime.JsonValue`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: parameter 2 is typed `$runtime.InputJsonObject`, without its fields

**DuckDB**: no. DuckDB isn't a Prisma datasource.

## sqlx-ts 0.49.0

**No codegen**: no. A run leaves new files to commit: `q.queries.ts`.

**Live database**: partly, 1 of 3. It connects, but types some expressions itself.

- postgres: `select add_one(id) as v from users`: it types the result `unknown`
- postgres: `select array_agg(id) as v from users`: it types the result `unknown`

**Nullable left joins**: no, 0 of 17

- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1`: `title` is typed `string`, but it can be null
- postgres: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: it reports an error: Error: [E014] Failed to find a table name from a FROM statement: statement: `Default FROM table is not found from the query: SELECT u.name, x.title FROM users u LEFT JOIN (SELECT author_id, title FROM posts) x ON x.author_id = u.id. Ensure your query has a valid FROM clause.`  L
- postgres: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u full join posts p on p.author_id = u.id`: `name` is typed `string`, but it can be null; `title` is typed `string`, but it can be null
- mysql: `select u.name, p.title from users u left join posts p on p.author_id = u.id`: `title` is typed `string`, but it can be null
- mysql: `select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title`: `title` is typed `string`, but it can be null
- mysql: `select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1`: `title` is typed `string`, but it can be null
- mysql: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: it reports an error: Error: [E014] Failed to find a table name from a FROM statement: statement: `Default FROM table is not found from the query: SELECT u.name, x.title FROM users u LEFT JOIN (SELECT author_id, title FROM posts) x ON x.author_id = u.id. Ensure your query has a valid FROM clause.`  L
- mysql: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, p.title from users u left join posts p on p.author_id = u.id`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: it reports an error: Error: [E014] Failed to find a table name from a FROM statement: statement: `Default FROM table is not found from the query: SELECT u.name, x.title FROM users u LEFT JOIN (SELECT author_id, title FROM posts) x ON x.author_id = u.id. Ensure your query has a valid FROM clause.`  L
- sqlite: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, p.title from users u full join posts p on p.author_id = u.id`: `name` is typed `string`, but it can be null; `title` is typed `string`, but it can be null

**Nullable expressions**: partly, 30 of 57

- postgres: `select max(id) as v from posts`: `v` is typed `number`, but it can be null
- postgres: `select sum(score) as v from posts`: `v` is typed `number`, but it can be null
- postgres: `select nickname || '!' as v from users`: `v` is typed `string`, but it can be null
- postgres: `select upper(nickname) as v from users`: `v` is typed `string`, but it can be null
- postgres: `select score + 1 as v from posts`: `v` is typed `number`, but it can be null
- postgres: `select nullif(name, 'x') as v from users`: `v` is typed `string`, but it can be null
- postgres: `select (select count(*) from posts) as v`: `v` is missing
- postgres: `select lag(id) over (order by id) as v from users`: `v` is typed `number`, but it can be null
- postgres: `select id as v from users union all select score from posts`: it reports an error: Error: Unknown error: `Unknown query type while processing query: SELECT id AS v FROM users UNION ALL SELECT score FROM posts`  Location:     src/ts_generator/generator.rs:199:3
- mysql: `select max(id) as v from posts`: `v` is typed `number`, but it can be null
- mysql: `select sum(score) as v from posts`: `v` is typed `number`, but it can be null
- mysql: `select concat(nickname, '!') as v from users`: `v` is typed `string`, but it can be null
- mysql: `select upper(nickname) as v from users`: `v` is typed `string`, but it can be null
- mysql: `select score + 1 as v from posts`: `v` is typed `number`, but it can be null
- mysql: `select nullif(name, 'x') as v from users`: `v` is typed `string`, but it can be null
- mysql: `select (select count(*) from posts) as v`: `v` is missing
- mysql: `select lag(id) over (order by id) as v from users`: `v` is typed `number`, but it can be null
- mysql: `select id as v from users union all select score from posts`: it reports an error: Error: Unknown error: `Unknown query type while processing query: SELECT id AS v FROM users UNION ALL SELECT score FROM posts`  Location:     src/ts_generator/generator.rs:199:3
- sqlite: `select max(id) as v from posts`: `v` is typed `number`, but it can be null
- sqlite: `select sum(score) as v from posts`: `v` is typed `number`, but it can be null
- sqlite: `select nickname || '!' as v from users`: `v` is typed `string`, but it can be null
- sqlite: `select upper(nickname) as v from users`: `v` is typed `string`, but it can be null
- sqlite: `select score + 1 as v from posts`: `v` is typed `number`, but it can be null
- sqlite: `select nullif(name, 'x') as v from users`: `v` is typed `string`, but it can be null
- sqlite: `select (select count(*) from posts) as v`: `v` is missing
- sqlite: `select lag(id) over (order by id) as v from users`: `v` is typed `number`, but it can be null
- sqlite: `select id as v from users union all select score from posts`: it reports an error: Error: Unknown error: `Unknown query type while processing query: SELECT id AS v FROM users UNION ALL SELECT score FROM posts`  Location:     src/ts_generator/generator.rs:199:3

**Parameter types**: partly, 16 of 17

- postgres: `select id from posts where created_at > $1`: parameter 1 is typed `unknown`, but it takes a timestamp

**Nullable parameters**: partly, 9 of 12

- postgres: `select id from users where nickname = $1`: parameter 1 is typed `string | null`, but it takes no null
- mysql: `select id from users where nickname = $1`: parameter 1 is typed `string | null`, but it takes no null
- sqlite: `select id from users where nickname = $1`: parameter 1 is typed `string | null`, but it takes no null

**Cardinality**: no, 0 of 28. It types the row only, and how many rows come back is left to you.

- postgres: `select id, name from users where id = $1`: it does not infer one
- postgres: `select id from users where email = $1`: it does not infer one
- postgres: `select count(*) as v from users`: it does not infer one
- postgres: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- postgres: `select id from users`: it does not infer one
- postgres: `select id from users limit 1`: it does not infer one
- postgres: `update users set name = $1 where id = $2 returning id`: it does not infer one
- postgres: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- postgres: `select id from posts where author_id = $1`: it does not infer one
- postgres: `delete from users where id = $1`: it does not infer one
- mysql: `select id, name from users where id = $1`: it does not infer one
- mysql: `select id from users where email = $1`: it does not infer one
- mysql: `select count(*) as v from users`: it does not infer one
- mysql: `select id from users`: it does not infer one
- mysql: `select id from users limit 1`: it does not infer one
- mysql: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- mysql: `select id from posts where author_id = $1`: it does not infer one
- mysql: `delete from users where id = $1`: it does not infer one
- sqlite: `select id, name from users where id = $1`: it does not infer one
- sqlite: `select id from users where email = $1`: it does not infer one
- sqlite: `select count(*) as v from users`: it does not infer one
- sqlite: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- sqlite: `select id from users`: it does not infer one
- sqlite: `select id from users limit 1`: it does not infer one
- sqlite: `update users set name = $1 where id = $2 returning id`: it does not infer one
- sqlite: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- sqlite: `select id from posts where author_id = $1`: it does not infer one
- sqlite: `delete from users where id = $1`: it does not infer one

**Nested JSON**: partly, 5 of 6

- sqlite: `select json_group_array(json_object('id', id, 'title', title)) as v from posts`: `v` is typed `unknown`, without its fields

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `object`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: parameter 2 is typed `object`, without its fields

**DuckDB**: no. Its `--db-type` takes Postgres, MySQL and SQLite.

## SafeQL 5.4.2

**No codegen**: partly. A run leaves changed source to commit: `right.ts`.

**Live database**: partly, 2 of 3. It connects, but types some expressions itself.

- postgres: `select add_one(id) as v from users`: it types the result `unknown | null`

**Nullable expressions**: partly, 15 of 19

- postgres: `select upper(nickname) as v from users`: `v` is typed `string`, but it can be null
- postgres: `select nullif(name, 'x') as v from users`: `v` is typed `boolean`, but it can be null
- postgres: `select lag(id) over (order by id) as v from users`: it reports an error: Query has incorrect type annotation. 	Expected: { v: any | null }[] 	  Actual: { v: anyelement | null }[]
- postgres: `select max(id) as v from posts group by author_id`: `v` is typed `number | null`, but it is never null

**Parameter types**: partly, 5 of 7

- postgres: `insert into posts (id, author_id, title, score) values ($1, $2, $3, $4)`: a boolean value passes for parameter 3, which takes text
- postgres: `select id from users where external_id = $1`: it reports an error: Invalid Query: operator does not exist: uuid = text

**Nullable parameters**: no, 0 of 4. It gives every parameter the same answer.

- postgres: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: a null value passes for parameter 1, which takes no null; a null value passes for parameter 2, which takes no null; a null value passes for parameter 3, which takes no null
- postgres: `update users set nickname = $1 where id = $2`: a null value passes for parameter 2, which takes no null
- postgres: `select name from users where id = $1`: a null value passes for parameter 1, which takes no null
- postgres: `select id from users where nickname = $1`: a null value passes for parameter 1, which takes no null

**Cardinality**: no, 0 of 10. Every query is typed as an array, unless your own transform says otherwise.

- postgres: `select id, name from users where id = $1`: it does not infer one
- postgres: `select id from users where email = $1`: it does not infer one
- postgres: `select count(*) as v from users`: it does not infer one
- postgres: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- postgres: `select id from users`: it does not infer one
- postgres: `select id from users limit 1`: it does not infer one
- postgres: `update users set name = $1 where id = $2 returning id`: it does not infer one
- postgres: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- postgres: `select id from posts where author_id = $1`: it does not infer one
- postgres: `delete from users where id = $1`: it does not infer one

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `any`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: it reports an error: Invalid Query: The type "{ x: number; }" has no corresponding PostgreSQL type. Please add it manually using the "overrides.types" option: ```ts {   "connections": {     ...,     "overrides": {       "types": {         "PG TYPE (e.g. 'date')": "{ x: number; }"       }     }   } }

**MySQL**: no. It is not available for MySQL.

**SQLite**: no. It is not available for SQLite.

**DuckDB**: no. SafeQL is Postgres only.

## sqlx 0.9.0

**No codegen**: yes. Nothing to commit after a run.

**Nullable left joins**: partly, 12 of 17

- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id`: `name` is typed `Option<TEXT>`, but it is never null; `title` is typed `TEXT`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title`: `title` is typed `TEXT`, but it can be null
- postgres: `select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1`: `title` is typed `TEXT`, but it can be null
- postgres: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: `name` is typed `Option<TEXT>`, but it is never null; `title` is typed `TEXT`, but it can be null
- postgres: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `name` is typed `Option<TEXT>`, but it is never null; `title` is typed `TEXT`, but it can be null

**Nullable expressions**: partly, 44 of 57

- postgres: `select count(*) as v from posts`: `v` is typed `Option<INT8>`, but it is never null
- postgres: `select coalesce(nickname, 'x') as v from users`: `v` is typed `Option<TEXT>`, but it is never null
- postgres: `select name || '!' as v from users`: `v` is typed `Option<TEXT>`, but it is never null
- postgres: `select upper(name) as v from users`: `v` is typed `Option<TEXT>`, but it is never null
- postgres: `select id + 1 as v from posts`: `v` is typed `Option<INT4>`, but it is never null
- postgres: `select nickname is null as v from users`: `v` is typed `Option<BOOL>`, but it is never null
- postgres: `select (select count(*) from posts) as v`: `v` is typed `Option<INT8>`, but it is never null
- postgres: `select exists (select 1 from posts) as v`: `v` is typed `Option<BOOL>`, but it is never null
- postgres: `select row_number() over (order by id) as v from users`: `v` is typed `Option<INT8>`, but it is never null
- postgres: `select max(id) as v from posts group by author_id`: `v` is typed `Option<INT4>`, but it is never null
- mysql: `select (select count(*) from posts) as v`: `v` is typed `Option<BIGINT>`, but it is never null
- mysql: `select max(id) as v from posts group by author_id`: `v` is typed `Option<INT>`, but it is never null
- sqlite: `select upper(name) as v from users`: `v` is typed `Option<TEXT>`, but it is never null

**Parameter types**: partly, 7 of 17

- mysql: `select name from users where id = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes a number
- mysql: `select id from users where name = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes text
- mysql: `insert into posts (id, author_id, title, score) values ($1, $2, $3, $4)`: parameter 1 is typed `any value (it counts arguments only)`, but it takes a number; parameter 2 is typed `any value (it counts arguments only)`, but it takes a number; parameter 3 is typed `any value (it counts arguments only)`, but it takes text; parameter 4 is typed `any value (it counts arguments
- mysql: `select id from users limit $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes a number
- mysql: `select id from users where name like $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes text
- sqlite: `select name from users where id = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes a number
- sqlite: `select id from users where name = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes text
- sqlite: `insert into posts (id, author_id, title, score) values ($1, $2, $3, $4)`: parameter 1 is typed `any value (it counts arguments only)`, but it takes a number; parameter 2 is typed `any value (it counts arguments only)`, but it takes a number; parameter 3 is typed `any value (it counts arguments only)`, but it takes text; parameter 4 is typed `any value (it counts arguments
- sqlite: `select id from users limit $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes a number
- sqlite: `select id from users where name like $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes text

**Nullable parameters**: no, 0 of 12. It gives every parameter the same answer.

- postgres: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 1 is typed `INT4 or Option<INT4>`, but it takes no null; parameter 2 is typed `TEXT or Option<TEXT>`, but it takes no null; parameter 3 is typed `TEXT or Option<TEXT>`, but it takes no null
- postgres: `update users set nickname = $1 where id = $2`: parameter 2 is typed `INT4 or Option<INT4>`, but it takes no null
- postgres: `select name from users where id = $1`: parameter 1 is typed `INT4 or Option<INT4>`, but it takes no null
- postgres: `select id from users where nickname = $1`: parameter 1 is typed `TEXT or Option<TEXT>`, but it takes no null
- mysql: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 1 is typed `any value (it counts arguments only)`, but it takes no null; parameter 2 is typed `any value (it counts arguments only)`, but it takes no null; parameter 3 is typed `any value (it counts arguments only)`, but it takes no null
- mysql: `update users set nickname = $1 where id = $2`: parameter 2 is typed `any value (it counts arguments only)`, but it takes no null
- mysql: `select name from users where id = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes no null
- mysql: `select id from users where nickname = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes no null
- sqlite: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 2 is typed `any value (it counts arguments only)`, but it takes no null; parameter 3 is typed `any value (it counts arguments only)`, but it takes no null
- sqlite: `update users set nickname = $1 where id = $2`: parameter 2 is typed `any value (it counts arguments only)`, but it takes no null
- sqlite: `select name from users where id = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes no null
- sqlite: `select id from users where nickname = $1`: parameter 1 is typed `any value (it counts arguments only)`, but it takes no null

**Cardinality**: no, 0 of 28. You pick it on each call: `fetch_one`, `fetch_optional` or `fetch_all`.

- postgres: `select id, name from users where id = $1`: it does not infer one
- postgres: `select id from users where email = $1`: it does not infer one
- postgres: `select count(*) as v from users`: it does not infer one
- postgres: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- postgres: `select id from users`: it does not infer one
- postgres: `select id from users limit 1`: it does not infer one
- postgres: `update users set name = $1 where id = $2 returning id`: it does not infer one
- postgres: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- postgres: `select id from posts where author_id = $1`: it does not infer one
- postgres: `delete from users where id = $1`: it does not infer one
- mysql: `select id, name from users where id = $1`: it does not infer one
- mysql: `select id from users where email = $1`: it does not infer one
- mysql: `select count(*) as v from users`: it does not infer one
- mysql: `select id from users`: it does not infer one
- mysql: `select id from users limit 1`: it does not infer one
- mysql: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- mysql: `select id from posts where author_id = $1`: it does not infer one
- mysql: `delete from users where id = $1`: it does not infer one
- sqlite: `select id, name from users where id = $1`: it does not infer one
- sqlite: `select id from users where email = $1`: it does not infer one
- sqlite: `select count(*) as v from users`: it does not infer one
- sqlite: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it does not infer one
- sqlite: `select id from users`: it does not infer one
- sqlite: `select id from users limit 1`: it does not infer one
- sqlite: `update users set name = $1 where id = $2 returning id`: it does not infer one
- sqlite: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it does not infer one
- sqlite: `select id from posts where author_id = $1`: it does not infer one
- sqlite: `delete from users where id = $1`: it does not infer one

**Nested JSON**: no, 0 of 6

- postgres: `select json_build_object('id', id, 'name', name) as v from users`: `v` is typed `Option<JSON>`, without its fields
- postgres: `select json_agg(json_build_object('id', id, 'title', title)) as v from posts`: `v` is typed `Option<JSON>`, without its fields
- mysql: `select json_object('id', id, 'name', name) as v from users`: `v` is typed `Option<JSON>`, without its fields
- mysql: `select json_arrayagg(json_object('id', id, 'title', title)) as v from posts`: `v` is typed `Option<JSON>`, without its fields
- sqlite: `select json_object('id', id, 'name', name) as v from users`: `v` is typed `Option<TEXT>`, without its fields
- sqlite: `select json_group_array(json_object('id', id, 'title', title)) as v from posts`: `v` is typed `Option<TEXT>`, without its fields

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `JSONB`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: parameter 2 is typed `JSONB or Option<JSONB>`, without its fields

**DuckDB**: no. sqlx has no DuckDB driver.

## TypeSQL 0.24.0

**No codegen**: no. A run leaves new files to commit: `sql/index.ts`, `sql/q0.ts`, `sql/q1.ts`.

**Nullable left joins**: partly, 11 of 17

- postgres: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `title` is typed `string`, but it can be null
- postgres: `select u.name, p.title from users u full join posts p on p.author_id = u.id`: `name` is typed `string`, but it can be null; `title` is typed `string`, but it can be null
- mysql: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `name` is typed `string | null`, but it is never null; `title` is typed `string`, but it can be null
- sqlite: `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, p.title from posts p right join users u on u.id = p.author_id`: `title` is typed `string`, but it can be null
- sqlite: `select u.name, p.title from users u full join posts p on p.author_id = u.id`: `name` is typed `string`, but it can be null; `title` is typed `string`, but it can be null

**Nullable expressions**: partly, 49 of 57

- postgres: `select upper(name) as v from users`: `v` is typed `string | null`, but it is never null
- postgres: `select (select count(*) from posts) as v`: `v` is typed `string | null`, but it is never null
- postgres: `select max(id) as v from posts group by author_id`: `v` is typed `number | null`, but it is never null
- mysql: `select (select count(*) from posts) as v`: `v` is typed `number | null`, but it is never null
- mysql: `select max(id) as v from posts group by author_id`: `v` is typed `number | null`, but it is never null
- sqlite: `select max(id) as v from posts`: `v` is typed `number`, but it can be null
- sqlite: `select (select count(*) from posts) as v`: `v` is typed `number | null`, but it is never null
- sqlite: `select id as v from users union all select score from posts`: `v` is typed `number`, but it can be null

**Parameter types**: partly, 16 of 17

- postgres: `select id from users limit $1`: parameter 1 is typed `string`, but it takes a number

**Cardinality**: partly, 16 of 28

- postgres: `select id from users where email = $1`: it says many rows, but this returns at most one row
- postgres: `select count(*) as v from users`: it says at most one row, but this returns exactly one row
- postgres: `update users set name = $1 where id = $2 returning id`: it says exactly one row, but this returns at most one row
- postgres: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it says many rows, but this returns at most one row
- mysql: `select count(*) as v from users`: it says at most one row, but this returns exactly one row
- mysql: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it says many rows, but this returns at most one row
- sqlite: `select id, name from users where id = $1`: it says many rows, but this returns at most one row
- sqlite: `select id from users where email = $1`: it says many rows, but this returns at most one row
- sqlite: `select count(*) as v from users`: it says at most one row, but this returns exactly one row
- sqlite: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it says at most one row, but this returns exactly one row
- sqlite: `update users set name = $1 where id = $2 returning id`: it says many rows, but this returns at most one row
- sqlite: `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: it says many rows, but this returns at most one row

**Nested JSON**: partly, 2 of 6

- mysql: `select json_object('id', id, 'name', name) as v from users`: it reports an error: it marks this query as invalid SQL
- mysql: `select json_arrayagg(json_object('id', id, 'title', title)) as v from posts`: it reports an error: it marks this query as invalid SQL
- sqlite: `select json_object('id', id, 'name', name) as v from users`: it reports an error: it marks this query as invalid SQL
- sqlite: `select json_group_array(json_object('id', id, 'title', title)) as v from posts`: it reports an error: it marks this query as invalid SQL

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `any`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: parameter 2 is typed `any`, without its fields

**DuckDB**: no. Its clients are Postgres, MySQL, SQLite, LibSQL and D1.

## Postgres Typed SQL 0.1.0-beta.22

**No codegen**: partly. By default it writes files to commit. An experimental workflow on TypeScript 7.1 types the import with nothing generated.

**Live database**: no, 3 of 3. It loads your schema file into an embedded Postgres, and never connects to your database.

**Nullable parameters**: no, 2 of 4. It gives every parameter the same answer.

- postgres: `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`: parameter 4 is typed `string`, but it takes null
- postgres: `update users set nickname = $1 where id = $2`: parameter 1 is typed `string`, but it takes null

**Cardinality**: partly, 9 of 10

- postgres: `insert into users (id, email, name) values ($1, $2, $3) returning id`: it says many rows, but this returns exactly one row

**JSON Schema types**: no, 0 of 2

- postgres: `select payload from events`: `payload` is typed `DbJsonSelected`, without its fields
- postgres: `insert into events (id, payload) values ($1, $2)`: parameter 2 is typed `DbJsonParameter`, without its fields

**MySQL**: no. It is not available for MySQL.

**SQLite**: no. It is not available for SQLite.

**DuckDB**: no. It is Postgres only.

## The queries

Every tool gets each query on each database it supports. [probes.ts](probes.ts) holds them with their right answers.

**outer-joins**

- `select u.name, p.title from users u left join posts p on p.author_id = u.id`
- `select u.name, p.title from users u left join posts p on p.author_id = u.id order by p.title`
- `select u.name, p.title from users u left join posts p on p.author_id = u.id where u.id = $1`
- `select u.name, x.title from users u left join (select author_id, title from posts) x on x.author_id = u.id`
- `select u.name, p.title from posts p right join users u on u.id = p.author_id`
- `select u.name, p.title from users u full join posts p on p.author_id = u.id` (Postgres and SQLite)

**expressions**

- `select count(*) as v from posts`
- `select max(id) as v from posts`
- `select sum(score) as v from posts`
- `select coalesce(nickname, 'x') as v from users`
- `select name || '!' as v from users` (Postgres and SQLite)
- `select concat(name, '!') as v from users` (MySQL)
- `select nickname || '!' as v from users` (Postgres and SQLite)
- `select concat(nickname, '!') as v from users` (MySQL)
- `select upper(name) as v from users`
- `select upper(nickname) as v from users`
- `select id + 1 as v from posts`
- `select score + 1 as v from posts`
- `select nullif(name, 'x') as v from users`
- `select case when id > 0 then name end as v from users`
- `select nickname is null as v from users`
- `select (select count(*) from posts) as v`
- `select exists (select 1 from posts) as v`
- `select row_number() over (order by id) as v from users`
- `select lag(id) over (order by id) as v from users`
- `select id as v from users union all select score from posts`
- `select max(id) as v from posts group by author_id`

**parameter-types**

- `select name from users where id = $1`
- `select id from users where name = $1`
- `insert into posts (id, author_id, title, score) values ($1, $2, $3, $4)`
- `select id from users limit $1`
- `select id from users where name like $1`
- `select id from users where external_id = $1` (Postgres)
- `select id from posts where created_at > $1` (Postgres)

**parameter-nullability**

- `insert into users (id, email, name, nickname) values ($1, $2, $3, $4)`
- `update users set nickname = $1 where id = $2`
- `select name from users where id = $1`
- `select id from users where nickname = $1`

**cardinality**

- `select id, name from users where id = $1`: at most one row
- `select id from users where email = $1`: at most one row
- `select count(*) as v from users`: exactly one row
- `insert into users (id, email, name) values ($1, $2, $3) returning id` (Postgres and SQLite): exactly one row
- `select id from users`: many rows
- `select id from users limit 1`: at most one row
- `update users set name = $1 where id = $2 returning id` (Postgres and SQLite): at most one row
- `select p.title from posts p join users u on u.id = p.author_id where p.id = $1`: at most one row
- `select id from posts where author_id = $1`: many rows
- `delete from users where id = $1`: no rows

**json-shapes**

- `select json_build_object('id', id, 'name', name) as v from users` (Postgres)
- `select json_object('id', id, 'name', name) as v from users` (MySQL and SQLite)
- `select json_agg(json_build_object('id', id, 'title', title)) as v from posts` (Postgres)
- `select json_arrayagg(json_object('id', id, 'title', title)) as v from posts` (MySQL)
- `select json_group_array(json_object('id', id, 'title', title)) as v from posts` (SQLite)

**json-schema**

- `select payload from events` (Postgres)
- `insert into events (id, payload) values ($1, $2)` (Postgres)

**from-the-database**

- `select add_one(id) as v from users` (Postgres)
- `select id::numeric / 2 as v from users` (Postgres)
- `select array_agg(id) as v from users` (Postgres)
