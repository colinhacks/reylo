create type mood as enum ('a', 'b'); create table t (id integer primary key, m varchar, check (m::mood <> 'b')); create table c (id integer primary key, t_id integer references t (id));
