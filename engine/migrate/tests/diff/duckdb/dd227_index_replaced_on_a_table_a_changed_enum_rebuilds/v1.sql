create type mood as enum ('a', 'b'); create table t (id integer primary key, m varchar, n integer, check (m::mood <> 'b')); create index t_i on t (n);
