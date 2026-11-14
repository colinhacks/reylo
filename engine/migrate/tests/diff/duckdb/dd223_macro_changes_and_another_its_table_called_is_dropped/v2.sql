create macro x() as 2; create table t (id integer primary key, v integer, check (v >= x()));
