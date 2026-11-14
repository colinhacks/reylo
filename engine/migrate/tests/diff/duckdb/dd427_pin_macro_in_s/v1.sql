create schema s; create type s.mood as enum ('a', 'b'); create macro s.m(x) as x + 1; create table t (a integer default s.m(1), b varchar, check (b::s.mood is not null));
