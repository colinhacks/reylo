create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, m mood, s varchar, check (s::mood <> 'b')); create index t_m on t (m);
