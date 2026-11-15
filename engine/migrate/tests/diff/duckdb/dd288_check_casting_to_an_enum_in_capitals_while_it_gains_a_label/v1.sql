create type mood as enum ('a', 'b'); create table t (id integer primary key, s varchar check (s::MOOD <> 'b'));
