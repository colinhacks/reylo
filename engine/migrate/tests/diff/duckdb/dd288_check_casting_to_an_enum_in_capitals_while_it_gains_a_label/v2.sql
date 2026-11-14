create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, s varchar check (s::MOOD <> 'b'));
