# sqlparser, the MySQL engine's copy

This is [`sqlparser`](https://github.com/apache/datafusion-sqlparser-rs) 0.63.0, the Apache DataFusion SQL parser, as published on crates.io, under the Apache License 2.0 (`LICENSE.TXT`). The MySQL engine reads a statement's tree with it for what the server's prepare metadata does not say: the row-count proof, the JSON constructors' shapes and the nullability probes.

The manifest leaves out the crate's development dependencies, its optional dependencies (the features stay, as names, so the sources' `cfg`s are known) and its docs.rs metadata, and its default features are `std` alone. Its sources are upstream's without the `#[cfg(test)]` items, which need those development dependencies.

## What this copy changes

The engine reads a query, INSERT (or REPLACE), UPDATE or DELETE, and parses nothing else, and its only dialect is MySQL's. The copy is cut to that, which takes the MySQL engine's artifact from 2.67 to 1.69 MB with every analysis unchanged. Each file it changes says so under its license header.

- `Statement` keeps the variants `Query`, `Insert`, `Update` and `Delete`, and is `#[non_exhaustive]`. Every match arm, parse function, conversion and dialect hook that named another variant is gone, and so is what only they used: through `SetExpr`, which can hold a `Statement`, every DDL type was reachable from a query, with its printing, cloning, comparing and dropping.
- `Parser::parse_statement` reads those statements alone, and a query can no longer be a MERGE.
- `dialect_of!` and `dialect_is!` are decided for `MySqlDialect` at compile time, so the parser's branches for other dialects are compiled out. The engine's dialect reports MySqlDialect's `TypeId`, as every dialect check saw before.
- A `Keyword` prints from `ALL_KEYWORDS` instead of a 65 KB match with an arm per keyword.
- `INSERT` no longer reads Hive's `OVERWRITE [LOCAL] DIRECTORY`, so `INSERT INTO directory …` and `INSERT INTO local …`, which upstream refuses, name a table as they do in MySQL.

## Where it reads MySQL as MySQL does

Measured against MySQL 8.4 itself: every statement of the kinds the engine reads in MySQL's own test suite (`mysql-test/t` of the 8.4 branch, 54,848 statements) prepared on a server and parsed by this copy, and each statement both accept printed back and prepared again. Upstream refused 964 of the 54,648 the server accepts and printed 94 back in a form the server read otherwise; this copy refuses 226 and prints 3 back so.

- `a JOIN b RIGHT JOIN c ON x` is `(a JOIN b) RIGHT JOIN c ON x`: a join with no ON or USING of its own is the left operand of the joins after it, and nests them only where an ON or USING is left over for it (`a JOIN b JOIN c ON x ON y` is `a JOIN (b JOIN c ON x) ON y`). Upstream nested every such join, which null-extended the wrong tables.
- A comma between table references binds looser than any JOIN, and is read as a CROSS JOIN of each later reference, in parentheses where it has joins of its own (`a, b RIGHT JOIN c ON x` never null-extends `a`): in a multiple-table `UPDATE t1, t2 SET …`, and in parentheses, `FROM (t1, t2)`. A lone table may stand in parentheses, `LEFT JOIN (t2) ON …`, and a join in parentheses takes no alias.
- `col->'path'` and `col->>'path'` bind tighter than any operator (`c->'$.a' + 1` adds 1 to the extracted value), and `!x` is NOT, bound as tightly; `a MOD b` is `a % b`; `<@x` is `<` and the variable `@x`.
- MySQL's reserved words (`information_schema.KEYWORDS`) are never an alias without `AS`, so `INSERT … SELECT 5 ON DUPLICATE KEY UPDATE …` reads its ON.
- `MATCH a AGAINST (…)` takes its columns with or without parentheses; `INTERVAL(N, N1, …)` is the function; `DELETE t1.*, t2.* FROM …` and `DELETE FROM t1.* USING …` read their targets; `CAST(x AS CHAR CHARACTER SET cs)` and `CONVERT(x, CHAR UNICODE)` keep their character set (`charset` on `Expr::Cast`; `ASCII`, `UNICODE` and `BINARY` stand for `latin1`, `ucs2` and `binary`); `CHAR(n USING cs)` keeps it as a `FunctionArgumentClause::Using`.
- A statement's `INTO` may follow the rest of it, before or after its locking clause, and is held on its SELECT; a subquery takes none. `LOCK IN SHARE MODE` is `FOR SHARE`, and `FOR SHARE OF t1, t2` keeps every table (`LockClause::of` is a list).
- A string literal prints its value as MySQL reads it back: the quote doubled, and a backslash and NUL escaped where the session reads backslash escapes (`set_backslash_escapes`, which the engine sets for the session it reads). A hexadecimal literal with an odd number of digits prints with a leading zero, `0x5ff` as `X'05ff'`.
- A stray `END` after a statement is an error, not the end of its text.
