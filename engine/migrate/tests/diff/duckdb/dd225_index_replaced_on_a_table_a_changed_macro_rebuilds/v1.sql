create macro x() as 1; create table t (id integer primary key, v integer, n integer, check (v >= x())); create index t_i on t (n);
