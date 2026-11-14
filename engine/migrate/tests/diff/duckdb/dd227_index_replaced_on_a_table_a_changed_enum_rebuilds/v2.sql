create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, m varchar, n integer, check (m::mood <> 'b')); create index t_i on t (m);
