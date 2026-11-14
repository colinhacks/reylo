create macro norm(s) as lower(s); create table t (id integer primary key, s varchar, n integer); create index t_norm on t (norm(s));
