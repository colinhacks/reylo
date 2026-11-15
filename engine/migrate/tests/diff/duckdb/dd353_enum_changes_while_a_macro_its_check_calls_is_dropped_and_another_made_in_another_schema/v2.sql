create schema b; create type mood as enum ('a', 'b', 'c'); create macro b.valid(s) as s <> ''; create table t (id integer primary key, m varchar check (b.valid(m) and m::mood <> 'b'));
