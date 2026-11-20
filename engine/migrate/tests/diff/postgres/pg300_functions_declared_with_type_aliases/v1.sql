create function f(a int8) returns int8 language sql immutable as $$ select a $$;
create function f(a int2) returns int2 language sql immutable as $$ select a $$;
