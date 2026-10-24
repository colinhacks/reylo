-- A domain named `lower`, over varchar, beside columns of a domain over text: the describe reports
-- such a column as a `text`, for which `lower(text)` is an exact match, but the server sees the
-- domain, finds none, and reads `lower(d)` as a cast to the type `lower`, whose own `||` answers
-- NULL for anything and whose `=` is true for anything, NULL included. A user's `lower(int8)`
-- lists the built-in `lower(text)` among the trust query's rows. Any `lower(x)` of one argument
-- proves nothing here, a plain text column's included.
create domain lower as varchar;
create function lower_cat(lower, lower) returns text language sql immutable as $$ select null::text $$;
create operator || (leftarg = lower, rightarg = lower, function = lower_cat);
create function lower_eq(lower, lower) returns bool language sql immutable as $$ select true $$;
create operator = (leftarg = lower, rightarg = lower, function = lower_eq);
create function lower(int8) returns text language sql immutable as $$ select null::text $$;
create domain mytext as text;
create table t (id int primary key, d mytext not null, n mytext, c text not null);
