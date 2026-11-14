create schema s; create type s.mood as enum ('a', 'b', 'c'); create macro s.m(x) as x + 2; create table t (a integer default s.m(1), b varchar, check (b::s.mood is not null));
