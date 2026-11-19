create function add_int(integer, integer) returns integer language sql immutable as $$ select $1 + $2 $$;
create function sub_int(integer, integer) returns integer language sql immutable as $$ select $1 - $2 $$;
create function larger_int(integer, integer) returns integer language sql immutable as $$ select greatest($1, $2) $$;
create aggregate mysum(integer) (sfunc = add_int, stype = integer, initcond = '0');
create aggregate count_all(*) (sfunc = pg_catalog.int8inc, stype = int8, initcond = '0');
create table kept (id integer primary key);
