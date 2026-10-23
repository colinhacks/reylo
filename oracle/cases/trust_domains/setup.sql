-- A domain over text and one over int4, with an `=`, a `+`, a `||`, a `lower` and a `<` of their
-- own: the describe reports a column of a domain as of its base type, which the server matches as
-- the domain. Each answers NULL for any input, the `<` true for any, NULL included.
create domain mytext as text;
create domain myint as int4;
create function mytext_eq(mytext, mytext) returns bool language sql immutable as $$ select null::bool $$;
create operator = (leftarg = mytext, rightarg = mytext, function = mytext_eq);
create function lower(mytext) returns text language sql immutable as $$ select null::text $$;
create function myint_plus(myint, int4) returns int4 language sql immutable as $$ select null::int4 $$;
create operator + (leftarg = myint, rightarg = int4, function = myint_plus);
create function mytext_cat(mytext, mytext) returns text language sql immutable as $$ select null::text $$;
create operator || (leftarg = mytext, rightarg = mytext, function = mytext_cat);
create function mytext_lt(mytext, mytext) returns bool language sql immutable as $$ select true $$;
create operator < (leftarg = mytext, rightarg = mytext, function = mytext_lt);
create table t (id int primary key, d mytext not null, n mytext, i myint not null, plain text not null);
create table posts (id int primary key, author_id int, title text not null);
