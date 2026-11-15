create macro x() as 1; create table t (id integer primary key, v integer, check (v >= x()));
