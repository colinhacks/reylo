create type mood as enum ('a', 'b', 'c'); create table t (id integer primary key, m varchar default 'a'::mood, check (m::main.mood <> 'b'));
