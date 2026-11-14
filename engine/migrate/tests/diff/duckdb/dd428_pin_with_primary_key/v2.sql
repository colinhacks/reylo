create schema s; create type s.mood as enum ('a', 'b', 'c'); create macro m(x) as x + 2; create table t (id integer primary key, a integer default m(1), b varchar, check (b::s.mood is not null));
