create macro norm(s) as lower(trim(s)); create table t (id integer primary key, s varchar, n integer check (n > 0)); create index t_norm on t (norm(s));
