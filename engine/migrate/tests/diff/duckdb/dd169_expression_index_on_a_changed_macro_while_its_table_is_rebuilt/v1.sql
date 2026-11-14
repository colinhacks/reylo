create macro lo(x) as lower(x); create table t (id integer primary key, s varchar); create index t_s on t (lo(s));
