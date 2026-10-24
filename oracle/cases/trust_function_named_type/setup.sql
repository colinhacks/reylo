-- The same with a base type named `lower`, which only a superuser creates, beside columns of a
-- domain over text: `lower(d)` is a cast to it.
create type lower;
create function lower_in(cstring) returns lower language internal immutable strict as 'textin';
create function lower_out(lower) returns cstring language internal immutable strict as 'textout';
create type lower (input = lower_in, output = lower_out, like = text);
create function lower_cat(lower, lower) returns text language sql immutable as $$ select null::text $$;
create operator || (leftarg = lower, rightarg = lower, function = lower_cat);
create function lower_eq(lower, lower) returns bool language sql immutable as $$ select true $$;
create operator = (leftarg = lower, rightarg = lower, function = lower_eq);
create function lower(int8) returns text language sql immutable as $$ select null::text $$;
create domain mytext as text;
create table t (id int primary key, d mytext not null, n mytext, c text not null);
