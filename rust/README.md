# Reylo for Rust: a prototype

`#[reylo::query("…")]` types a Postgres, MySQL or SQLite statement with Reylo's engine at compile time and fills the module under it with the statement's types. The query runs on sqlx, on the app's own pool, connection or transaction. The scheme of `DATABASE_URL` names the database: `postgres://` or `postgresql://`, `mysql://`, else `sqlite:` or `file:` with a path relative to the crate.

```rust
#[reylo::query("select id, name, nickname from users where id = :id")]
pub mod get_user {
    impl Row {
        pub fn display_name(&self) -> &str {
            self.nickname.as_deref().unwrap_or(&self.name)
        }
    }
}

let user = get_user::Query { id: 2 }.run(&pool).await?;   // Option<get_user::Row>

pub async fn find(pool: &sqlx::PgPool, id: i32) -> sqlx::Result<Option<get_user::Row>> {
    get_user::Query { id }.run(pool).await
}
```

The module holds:

- `Query`, the statement's parameters, one field per `:name`, a text parameter a `&str`. Its `.run(executor)` returns what the statement's cardinality says: `Row` for exactly one row, `Option<Row>` for at most one, `Vec<Row>` for any number, and the number of rows changed for a write that returns none. A statement with no parameters has a unit `Query`: `list_users::Query.run(&pool)`. The struct is also named `Params`, the name compiler errors use.
- `Row`, one row, which derives `Debug`, `Clone` and `PartialEq`, and serde's `Serialize` unless a column's Rust type has no serde form (Postgres `interval`, `money` and `oid`, MySQL `TIME`). A write that returns no rows has none.
- An enum for each Postgres enum type the statement reads or binds (`users_by_mood::Mood`), and a struct for each JSON object it builds.
- Whatever the crate writes inside the braces, which sees the items of the module around it.

A misspelt, missing or mistyped parameter is Rust's own error on the struct literal (``struct `get_user::Params` has no field named `user_id` ``, with the fields it has). A statement the database refuses is one error at the statement, and the items written inside its module add none of their own.

| Crate | What it is |
| --- | --- |
| `reylo-macros` | The attribute macro: the engine run against `DATABASE_URL` (or `reylo/.cache/` where the database cannot be reached), connected as the engine connects, TLS included, filling the module with `Query` (also named `Params`), `Row` and `run` |
| `reylo-sqlx` | The run time, which a crate depends on as `reylo` (`reylo = { package = "reylo-sqlx", … }`, since the engine's binary is the package `reylo`): the macro, `Exact`, and the crates the expansion names; its features `postgres` (the default), `mysql` and `sqlite` turn on sqlx's drivers, and `chrono`, `time`, `uuid`, `rust_decimal` and `bigdecimal` the Rust types of dates, uuids and decimals, with their serde support |
| `demo-reylo`, `demo-sqlx` | The same seven statements through `#[reylo::query]` and `sqlx::query!`, and in `demo-reylo` more: an enum, a timestamptz, a uuid, a numeric and an interval; an enum parameter; rows held in a struct of the crate's own and serialized; a write |
| `demo-mysql` | The statements on MySQL, with an unsigned BIGINT, a BOOLEAN, an ENUM, a TIMESTAMP, a DATE, a TIME, a DECIMAL, a YEAR, a BIT, a BLOB and two writes |
| `demo-sqlite` | The statements on SQLite, from `demo.db`, with a boolean, a 64-bit integer, a blob and two writes |

## How a column gets its Rust type

The engine describes each parameter and column in the type descriptor format (`spec/`). The macro reads the kind and the fields it requires (an integer's width and sign, a float's widths), and the database's label for the type where a client's convention reads it, as the TypeScript loader does: a SQLite column declared BOOLEAN is a `bool`, and MySQL's `tinyint(1)` too. A column that may be null is an `Option`, and an array whose elements may be NULL a `Vec<Option<T>>`.

Every read is exact or an error, never another value. A Postgres `date`, `timestamp` and `timestamptz` may hold infinity and years chrono and time cannot write, and a `time` holds `24:00:00`; sqlx's decoders panic on the first two and turn the third into `00:00:00`. The macro decodes those columns through `reylo::Exact`, which refuses such a value with an error naming the column, as sqlx already refuses a `numeric`'s NaN. Where no Rust type holds a column's values (a decimal inside JSON, which serde reads as an `f64`; a SQLite column declared NUMERIC; a value of no known type), the expansion fails at the statement and says how to cast it.

## Running the demos

A throwaway Postgres (any port but 5432), with this schema in a database `demo`:

```sql
create type mood as enum ('ok', 'sad');
create table users (id int primary key, name text not null, nickname text, mood mood, joined timestamptz not null default now(), token uuid not null default gen_random_uuid(), balance numeric(10, 2) not null default 0);
create table posts (id serial primary key, author_id int not null references users (id), title text not null, tags text[] not null default '{}');
insert into users (id, name, nickname, mood) values (1, 'ada', 'countess', 'ok'), (2, 'bob', null, 'sad');
insert into posts (author_id, title, tags) values (1, 'hello', '{intro}'), (1, 'again', '{intro,null}');
```

A throwaway MySQL 8.4 (any port but 3306) with `rust/demo-mysql/schema.sql` in a database `demo`, and the SQLite file:

```sh
sqlite3 rust/demo-sqlite/demo.db < rust/demo-sqlite/schema.sql
```

One `DATABASE_URL` in the environment names one database for every crate, so a build of every demo would analyze the SQLite demo's statements on Postgres. Give each demo its own in the crate's `.env`, which git ignores and which the macros (and sqlx's, for `demo-sqlx`) read when the environment has none. Then, after `engine/build.sh --prepare`:

```sh
echo DATABASE_URL=postgres://postgres@127.0.0.1:55840/demo | tee rust/demo-reylo/.env > rust/demo-sqlx/.env
echo DATABASE_URL=mysql://root@127.0.0.1:53306/demo > rust/demo-mysql/.env
echo DATABASE_URL=sqlite:demo.db > rust/demo-sqlite/.env
cargo build --all-targets -p demo-reylo -p demo-sqlx -p demo-mysql -p demo-sqlite
DATABASE_URL=postgres://postgres@127.0.0.1:55840/demo cargo run -p demo-reylo   # every statement typed and run
DATABASE_URL=postgres://postgres@127.0.0.1:55840/demo cargo run -p demo-sqlx    # statements 1 and 7 fail with "unexpected null"
DATABASE_URL=mysql://root@127.0.0.1:53306/demo cargo run -p demo-mysql
cargo run -p demo-sqlite
```

Where the database cannot be reached (the server down, the file gone) or no URL is set, the macro reads the analysis `reylo/.cache/` holds, so a crate built once builds again offline.
