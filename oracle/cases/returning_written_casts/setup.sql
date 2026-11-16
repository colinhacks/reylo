-- A value a write gives a column of another type reaches it through an assignment cast, which a
-- user may define, and which may give NULL: a column of a type such a cast targets holds what the
-- cast made of the value, and a string literal or a bare parameter, which the server reads as the
-- column's own type, takes no cast.
create table t (id int primary key, note text, n int8);
insert into t values (1, null, null);
create type boxed as (v int);
create function boxed_text(boxed) returns text language sql as 'select null::text';
create cast (boxed as text) with function boxed_text(boxed) as assignment;
