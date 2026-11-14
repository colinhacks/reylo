create schema b; create type mood as enum ('a', 'b'); create macro valid(s) as s <> ''; create table t (id integer primary key, m varchar check (valid(m) and m::mood <> 'b'));
