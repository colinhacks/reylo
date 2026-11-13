-- JSON a constructor builds is typed as that JSON (wiki/dialects.md, 2026-10-20): SQLite returns it
-- as text, which the run time parses, and the oracle holds each value inside it to the type the
-- engine gave it, as SQLite writes it there: an INTEGER (a BIGINT column's too, seeded with
-- SQLite's largest integer) a JSON integer, a REAL a number (`1e308` from the seeds, and an
-- infinity as `9.0e+999`), a TEXT a string, a NUMERIC either, NULL a null only where the type
-- says it may be, a truth value 1 or 0. A value nests as JSON only where it keeps SQLite's JSON
-- subtype; where it may not, its type is any JSON: an aggregate beside a window call, which SQLite
-- reads back from a subquery of its own, and a VIRTUAL generated column, which it computes where the
-- column is read. No statement reads the BLOB column into a
-- document: SQLite reads a BLOB as JSONB there, and the seeded empty blob is an error.
create table users (id integer primary key, name text not null, email text unique, github_id bigint, score real, active boolean not null default 1, extra, n numeric, doc text);
create table posts (id integer primary key, user_id integer not null references users (id), title text not null, body text, cover blob) strict;
create view titled as select id, user_id, json_object('id', id, 'title', title) as post from posts;
create table docs (id integer primary key, a integer not null, j text as (json_object('a', a)) virtual, js text as (json_object('a', a)) stored, jj text as (j) virtual, label text as ('#' || a) virtual) strict;
