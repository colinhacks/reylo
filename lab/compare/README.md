# Comparison

Runs every tool that infers types from hand-written SQL on the same queries, against real servers, and grades what each one answers. The marks in the docs' comparison come from here: nothing in the table is written by hand.

```sh
./run.sh                    # servers, every tool; writes results.json and RESULTS.md
./run.sh --only sqlc,reylo  # those tools only, merged into the results
```

It needs Homebrew's `postgresql@18` (with the JSON Schema validators) and `mysql@8.4`, or `PG18_BIN` and `MYSQL_BIN`; `sqlc` (`brew install sqlc`), rustup's toolchain and sqlite3. `SQLX_TS_BIN` names a `sqlx-ts` binary; without one, `run.sh` builds its tagged release. The servers take `COMPARE_PG_PORT` (57700) and `COMPARE_MYSQL_PORT` (57701), and `run.sh` stops them at the end.

## How a mark is computed

[probes.ts](probes.ts) holds the schema's queries and the right answer to each, and [grade.ts](grade.ts) turns a tool's answers into a mark. A tool's adapter in [tools/](tools/) runs the tool as a user would and reads back what it wrote: sqlc's Go, the TypeScript that PgTyped, Prisma and sqlx-ts generate, the row type SafeQL's ESLint fix writes, the description sqlx's macros read, and Reylo's analysis.

- A mark is yes when every query on every database the tool supports is right, no when none is, and partly otherwise.
- It is no as well when a tool gives every expression or parameter the same answer, since that reads nothing from the query, even though the answer is right for some.
- A tool's cardinality counts only where the tool works it out. A tool that has the user declare it (`:one`) or pick it per call (`fetch_one`) answers no.
- No codegen reads `git status` after a run: nothing to commit is yes, a changed source file is partly, new files are no. A tool with a second way that generates nothing, run for real, is partly.
- Live database runs the tool once with no server to reach, to see whether it needs one, and checks what it says of a function's result and an operator's.

Postgres's rows are seeded and analyzed, so a tool that reads the query plan sees the plan a real table gets. Unlike the other labs, this one installs `reylo` from npm, the release a user gets, and a run also writes `site/lib/compare-results.json`, which the docs' table reads.
