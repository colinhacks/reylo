create macro gen() as 42; create table t (id integer, n integer default gen());
