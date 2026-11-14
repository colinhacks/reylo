create macro x() as 2; create table t (id integer primary key, v integer, n integer, check (v >= x())); create index t_i on t (v);
