# reylo-engine-syntax

Lossless SQL syntax trees. Every byte of the text belongs to a token or to the trivia before one, so a tree prints back what it read, and invalid SQL still has a tree.

```rust
use reylo_engine_syntax::{parse, Dialect};

let tree = parse(Dialect::Postgres, "select id from users where");
assert_eq!(tree.print(), "select id from users where");
assert_eq!(tree.errors[0].message, "syntax error at end of input");
```

| Dialect | Read with | Errors |
| --- | --- | --- |
| Postgres | Tables from Postgres 18.6's `gram.y`, a port of `scan.l` | Postgres's own |
| SQLite | Tables from SQLite 3.53.4's `parse.y`, a port of `tokenize.c` | SQLite's own |
| MySQL, ClickHouse, SQL Server | The dialect's lexer, with statements, clauses and parentheses | What doesn't end or balance |

MySQL's grammar is GPL, so nothing here is generated or derived from it.

## Regenerating

Each `generated` file comes from a pinned upstream version, and is never edited.

```sh
python3 -I engine/syntax/gen/postgres.py --postgres <postgres> --libpg-query <libpg_query> --pg-query <pg_query.rs>
python3 -I engine/syntax/gen/sqlite.py --sqlite <sqlite-src-3530400> --amalgamation engine/sqlite/.sqlite/sqlite-amalgamation-3530400/sqlite3.c
```

Postgres's needs Bison 3.8.2.
