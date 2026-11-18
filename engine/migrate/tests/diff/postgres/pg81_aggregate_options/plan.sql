drop aggregate mysum(integer);
create aggregate mymax(integer) (sfunc = larger_int, stype = integer, sortop = operator(pg_catalog.>));
create aggregate mysum(integer) (sfunc = add_int, stype = integer, combinefunc = add_int, initcond = '0', msfunc = add_int, minvfunc = sub_int, mstype = integer, minitcond = '0', parallel = safe);
comment on aggregate count_all(*) is 'counts all rows';
