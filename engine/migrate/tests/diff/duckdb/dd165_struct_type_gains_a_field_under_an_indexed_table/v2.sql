create type rec as struct(a integer, b integer); create table t (id integer primary key, r rec, n integer); create index t_n on t (n);
