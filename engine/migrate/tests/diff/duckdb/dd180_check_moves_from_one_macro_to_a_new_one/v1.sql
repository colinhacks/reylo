create macro x(a) as a > 0; create table t (id integer primary key, v integer, check (x(v)));
