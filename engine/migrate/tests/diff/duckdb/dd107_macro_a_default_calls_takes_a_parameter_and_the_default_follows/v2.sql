create macro gen(x) as x + 7; create table t (id integer primary key, n integer default gen(1));
