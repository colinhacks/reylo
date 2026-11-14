create schema s; create type s.mood as enum ('a', 'b'); create macro m(x) as x + 1; create table t (id integer primary key, a integer default m(1), b varchar, check (b::s.mood is not null));
