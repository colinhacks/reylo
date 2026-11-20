set check_function_bodies = off;
drop function f(smallint);
create or replace function f(a int8) returns int8 language sql immutable as $$ select a + 1 $$;
create function f(a float8) returns float8 language sql immutable as $$ select a $$;
create function g(b bool) returns bool language sql immutable as $$ select not b $$;
reset check_function_bodies;
