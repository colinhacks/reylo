create schema s; create type s.mood as enum ('a', 'b'); create macro m(x) as x + 1; create table t (a integer default m(1), b varchar, check (cast(b as s.mood) is not null));
