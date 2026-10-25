-- An `=` of a user's, NULL for anything, over an enum of theirs: IS [NOT] DISTINCT FROM compares
-- two non-null values with it and is then NULL, where with a NULL on either side no `=` runs.
-- NULLIF calls the same `=` and claims nothing either way. Beside it the built-in `=` of two ints.
create type ue as enum ('a', 'b');
create function ue_null(ue, ue) returns bool language sql immutable as $$ select null::bool $$;
create operator = (leftarg = ue, rightarg = ue, function = ue_null);
create table e (id int primary key, a ue not null, b ue not null, n int);
