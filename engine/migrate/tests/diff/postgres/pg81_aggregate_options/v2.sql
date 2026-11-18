create function add_int(integer, integer) returns integer language sql immutable as $$ select $1 + $2 $$;
create function sub_int(integer, integer) returns integer language sql immutable as $$ select $1 - $2 $$;
create function larger_int(integer, integer) returns integer language sql immutable as $$ select greatest($1, $2) $$;
create aggregate mysum(integer) (sfunc = add_int, stype = integer, initcond = '0', combinefunc = add_int, msfunc = add_int, minvfunc = sub_int, mstype = integer, minitcond = '0', parallel = safe);
create aggregate mymax(integer) (sfunc = larger_int, stype = integer, sortop = >);
create aggregate count_all(*) (sfunc = pg_catalog.int8inc, stype = int8, initcond = '0');
comment on aggregate count_all(*) is 'counts all rows';
create table kept (id integer primary key);
