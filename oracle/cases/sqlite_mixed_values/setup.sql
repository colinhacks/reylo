-- A value SQLite hands back from one of several (a compound's arms, CASE, coalesce, iif, min and
-- max of several arguments, lag and lead with a default) takes the type they share, and is of no
-- known type where one may be of another storage class: a literal of another type, a column
-- declared without a type, json_extract. Every table here but u and dts is STRICT, so a value of
-- another type can only come from the statement. A DATE or DATETIME column of dts has NUMERIC
-- affinity and holds the text of a date, which a number beside it leaves as it is.
create table t (id integer primary key, n integer not null, s text, b blob) strict;
create table u (id integer primary key, x);
create view w as select 'x' as v union all select n from t;
create table dts (id integer primary key, d date not null, dt datetime, ok boolean not null);
insert into dts values (1, '2024-01-01', '2024-01-01 10:00:00', 1), (2, 20240101, null, 0);
