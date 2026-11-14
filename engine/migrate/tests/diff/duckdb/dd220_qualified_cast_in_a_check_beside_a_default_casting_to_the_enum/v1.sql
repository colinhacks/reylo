create type mood as enum ('a', 'b'); create table t (id integer primary key, m varchar default 'a'::mood, check (m::main.mood <> 'b'));
