create function f(s integer, k integer = 43) returns integer language sql as $$ select 1 $$;
create table t (c1 integer default f(1));
