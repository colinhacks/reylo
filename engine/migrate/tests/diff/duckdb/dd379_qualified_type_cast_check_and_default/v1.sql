create schema s; create type s.mood as enum ('a', 'b'); create table t (a varchar default ('a'::s.mood)::varchar, b varchar, check (b::s.mood is not null));
