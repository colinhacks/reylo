create macro lo(x) as lower(trim(x)); create table t (id integer primary key, s varchar check (s <> '')); create index t_s on t (lo(s));
