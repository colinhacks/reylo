# Reylo for Rust

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

- `Query`, the statement's parameters, one field per `:name`, a text parameter a `&str`. Its `.run(executor)` returns what the statement's cardinality says: `Row` for exactly one row, `Option<Row>` for at most one, `Vec<Row>` for any number, and the number of rows changed for a write that returns none; a second row where the cardinality allows one is an error, `reylo::UnexpectedRow`, never a row dropped. A query of any number of rows also has `.stream(executor)`, its rows one at a time. A statement with no parameters has a unit `Query`: `list_users::Query.run(&pool)`. The struct is also named `Params`, the name compiler errors use.
- `Row`, one row, which derives `Debug`, `Clone` and `PartialEq`, and serde's `Serialize` unless a column's Rust type has no serde form (Postgres `interval`, `money` and `oid`, MySQL `TIME`). A write that returns no rows has none.
- `SQL`, the statement as sqlx sends it.
- An enum for each database enum the statement reads or binds (`users_by_mood::Mood`): a Postgres enum type, a text column whose `CHECK` lists its values, a MySQL `ENUM`, or a list the statement names, as `case … then 'a' else 'b' end`. Its variants are its values in PascalCase (`send_email` and `send-email` are `SendEmail`), and a value that is no Rust name that way, or that would share its name with another, is spelled out after a `_` (`_1st`; `_send_5f_email` and `_send_2d_email` together), so no two values share a name and none depends on the values' order (`rust/reylo-build/src/names.rs`). Each implements `reylo::Enum`, whose `.cast()` turns one query's value into another's, and compiles only between enums of the same database enum.
- A struct for each JSON object the statement builds.
- Whatever the crate writes inside the braces, which sees the items of the module around it. A `pub struct Row { … }` with some of the row's fields gives those fields the crate's types: an enum of the crate's own for a column of a database enum, matched variant by variant (one it lacks, or one beyond the database's values, fails to compile), and any other type through `Into`. A `pub struct Params { … }` does the same for parameters, and `pub use crate::jobs::QueryJob as Row;` makes the crate's struct the whole row, an enum field through `reylo::Enum::cast`.

`reylo::migrate!()` compiles the project's migrations into the crate, with their lock and `reylo/schema.sql` (the `migrate` feature, a default one): `.run(&pool)` runs `reylo migrate deploy` with them on the pool's database and returns the names it ran, and `.status(&pool)` runs `reylo migrate status`. Both are the command's own code (`reylo-migrate` without its `cli` feature, so without the command line's clients, shadow databases or diff engine), over one connection of the pool's (`rust/reylo/src/migrate.rs`), and `rust/reylo/tests/migrate.rs` holds each against the command line on the same database.

`#[reylo::reflect("public.mood")]` on an enum of the crate's own, or `#[reylo::reflect("posts.status")]` for a column a `CHECK` or a MySQL `ENUM` lists values for (the database says which a name is; a name that is both is the type, and the column is written with its schema), checks its variants against the database's values, fills in an empty one, and implements `reylo::Enum` and sqlx's `Type`, `Encode` and `Decode` for it. In a file `reylo generate rust-sqlx --out` or `reylo-build` writes, each Postgres enum type is declared once, in a module named after its schema (`queries::public::Mood`, the public schema's also at the top, `queries::Mood`), which each query's module names as its own.

A misspelt, missing or mistyped parameter is Rust's own error on the struct literal (``struct `get_user::Params` has no field named `user_id` ``, with the fields it has). A statement the database refuses is one error at the statement, and the items written inside its module add none of their own.

| Crate | What it is |
| --- | --- |
| `reylo-macros` | The attribute macro: the engine run against `DATABASE_URL` (or `reylo/.cache/` where the database cannot be reached), connected as the engine connects, TLS included, filling the module with `Query` (also named `Params`), `Row` and `run` |
| `reylo` | The run time, which a crate depends on: the macro, `Exact`, and the crates the expansion names. Its features `sqlx_0_9` (the default) and `sqlx_0_8` name the sqlx it runs on; `postgres` (the default), `mysql` and `sqlite` turn on sqlx's drivers and the macro's engine and client for each, so a crate builds only its database's (SQLite on sqlx 0.9 alone); `migrate` (a default) `reylo::migrate!()`; and `chrono`, `time`, `uuid`, `rust_decimal` and `bigdecimal` the Rust types of dates, uuids and decimals, with their serde support. On sqlx 0.8 the crate turns on sqlx's own `json`, `chrono`, `time` and `uuid`, which reylo leaves alone there, since each names sqlx 0.8's SQLite driver |
| `reylo-build` | A statement's analysis as its module's items, which the macro expands to; the analysis itself, from the database or `reylo/.cache/` (`analyze`); the generator `reylo generate rust-sqlx` runs, which writes the modules to files (`generate`); and that generator for a build script: `reylo_build::rust_sqlx().input("src/queries").run()` writes every module into one file in `OUT_DIR`. Its default features, `generate` and `postgres`, are a build script's; `mysql` and `sqlite` add those databases |
| `demo-reylo` | Statements on Postgres: a left join, `count(*)`, `coalesce`, a key lookup, built JSON, an array, an enum, a timestamptz, a uuid, a numeric and an interval; an enum parameter; rows held in a struct of the crate's own and serialized; a write; a CHECK-listed column; the crate's own enum for a column and a parameter, and its struct as a whole row with a `reflect` enum; `.cast()`, `.stream()` and `SQL` |
| `demo-mysql` | The statements on MySQL, with an unsigned BIGINT, a BOOLEAN, an ENUM (a column and a parameter, one cast to the other), a TIMESTAMP, a DATE, a TIME, a DECIMAL, a YEAR, a BIT, a BLOB and two writes |
| `demo-sqlite` | The statements on SQLite, from `demo.db`, with a boolean, a 64-bit integer, a blob and two writes |
| `demo-docs` | Every Rust snippet of the docs site, as the pages write it, with the types the pages name written out, and `reylo::migrate!()` run on a database of its own with the demo's `reylo/migrations` |
| `demo-codegen` | Modules `reylo generate rust-sqlx` writes beside their `.sql` files, and one `reylo-build` writes from `build.rs` |

## How a column gets its Rust type

The engine describes each parameter and column in the type descriptor format (`spec/`). The macro reads the kind and the fields it requires (an integer's width and sign, a float's widths), and the database's label for the type where a client's convention reads it, as the TypeScript loader does: a SQLite column declared BOOLEAN is a `bool`, and MySQL's `tinyint(1)` too. A column that may be null is an `Option`, and an array whose elements may be NULL a `Vec<Option<T>>`.

Every read is exact or an error, never another value, except a `bool` read by a client's convention: a MySQL `TINYINT(1)` and a SQLite column declared BOOLEAN can hold 5, which sqlx reads as `true`, and SQLite turns any other value into an integer first, so `'yes'` and `0.5` read as `false`. A Postgres `numeric` read as a `rust_decimal::Decimal` and MySQL text of any collation go through `reylo::Exact` too, which on sqlx 0.8 refuses a numeric past a Decimal's range, where sqlx's own decoder panics, and reads text with a binary collation, which its `String` refuses. A Postgres `date`, `timestamp` and `timestamptz` may hold infinity and years chrono and time cannot write, and a `time` holds `24:00:00`; sqlx's decoders panic on the first two and turn the third into `00:00:00`. The macro decodes those columns through `reylo::Exact`, which refuses such a value with an error naming the column, as sqlx already refuses a `numeric`'s NaN. Where no Rust type holds a column's values (a decimal inside JSON, which serde reads as an `f64`; a SQLite column declared NUMERIC; a value of no known type), the expansion fails at the statement and says how to cast it.

## Running the demos

Each demo's statements are typed against its database as it compiles, so a build of the workspace leaves the demos out: each builds only with its `live` feature. `.github/scripts/rust-demos.sh` makes each demo's database from its `schema.sql`, on a throwaway Postgres and MySQL 8.4 (any ports but 5432 and 3306), and builds and runs every demo, as CI does:

```sh
REYLO_TEST_PG=postgres://postgres@127.0.0.1:55432/postgres REYLO_TEST_MYSQL=mysql://root@127.0.0.1:53306 .github/scripts/rust-demos.sh
```

One demo, with its own database:

```sh
DATABASE_URL=postgres://postgres@127.0.0.1:55432/rust_demo cargo run -p demo-reylo --features live
```

Where the database cannot be reached (the server down, the file gone) or no URL is set, the macro reads the analysis `reylo/.cache/` holds, so a crate built once builds again offline.

A crate's statements are compiled again when the project's `reylo/schema.sql`, its migrations' lock or its project file changes: the macro includes each in its expansion, so Cargo watches them.
