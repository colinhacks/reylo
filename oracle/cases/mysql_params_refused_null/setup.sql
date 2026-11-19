-- A column that may hold NULL by its declaration may still refuse it through a CHECK (MySQL
-- 8.0.16 and later). Until 2026-10-21 a bare parameter written whole to such a column was typed
-- nullable, and MySQL refused the NULL (error 3819). A bare parameter written whole takes the
-- column's nullability, read from its NOT NULL and its enforced CHECKs, so each one here is a value
-- but `note`'s; one cast to `nullable(…)` may be null. The oracle fails a case whose NULL the database refuses
-- with a NOT NULL (1048) or a CHECK (3819) error.
create table accounts (id int primary key, email varchar(40) check (email is not null), nickname varchar(20) check (nickname is not null), note varchar(40));
insert into accounts values (1, 'ada@example.com', 'ada', null);
-- A NOT NULL generated column refuses NULL in what it computes, so a column its expression is NULL
-- for wherever that column is NULL refuses it too (error 1048, naming the generated column), and a
-- column it reads through `coalesce` does not.
create table measures (id int primary key, nn int not null, nu int, g int generated always as (nu + 1) stored not null, nv int, h int generated always as (coalesce(nv, 0)) virtual not null, ns varchar(10), j varchar(12) generated always as (concat('x', ns)) virtual not null);
-- Each generated column of `computed` reads a column of its own. The expression is NULL wherever
-- that column is NULL through a function or operator measured strict in it (a JSON path, a string,
-- date or hash function, a comparison, TRIM and SUBSTRING), so the column refuses NULL; through
-- `concat_ws` after its separator, `ifnull`, or `json_set`'s value it may be NULL.
create table computed (id int primary key, j json, a text generated always as (json_unquote(json_extract(j, '$'))) virtual not null, k json, b text generated always as (k->>'$') virtual not null, e varchar(40), c varchar(40) generated always as (substring_index(e, '@', -1)) virtual not null, t varchar(20), d varchar(20) generated always as (lower(replace(t, ' ', '-'))) virtual not null, born date, f int generated always as (to_days(born)) virtual not null, pw varchar(20), h char(32) generated always as (md5(pw)) virtual not null, n int, positive boolean generated always as (n > 0) virtual not null, raw varchar(20), trimmed varchar(20) generated always as (trim(raw)) virtual not null, code varchar(20), prefix varchar(3) generated always as (substring(code, 1, 3)) virtual not null, due date, dom int generated always as (dayofmonth(due)) virtual not null, tag varchar(10), tags varchar(30) generated always as (concat_ws(',', 'x', tag)) virtual not null, hint varchar(10), shown varchar(10) generated always as (ifnull(hint, '')) virtual not null, m int, jm json generated always as (json_set('{}', '$.a', m)) virtual not null);
