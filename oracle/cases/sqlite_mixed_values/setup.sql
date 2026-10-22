-- A value SQLite hands back from one of several (a compound's arms, CASE, coalesce, iif, min and
-- max of several arguments, lag and lead with a default) takes the type they share, and is of no
-- known type where one may be of another storage class: a literal of another type, a column
-- declared without a type, json_extract. Every table here but u is STRICT, so a value of another
-- type can only come from the statement.
create table t (id integer primary key, n integer not null, s text, b blob) strict;
create table u (id integer primary key, x);
