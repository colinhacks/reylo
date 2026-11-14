create type mood as enum ('a', 'b', 'c'); create table t (id integer, m varchar, check (m::mood <> 'b'));
