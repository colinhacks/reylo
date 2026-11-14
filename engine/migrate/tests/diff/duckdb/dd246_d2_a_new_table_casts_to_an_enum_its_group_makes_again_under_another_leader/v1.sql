create type mood as enum ('a', 'b'); create macro x() as 1; create table t (id integer primary key, m varchar, v integer, check (v >= x() and m::mood <> 'b'));
