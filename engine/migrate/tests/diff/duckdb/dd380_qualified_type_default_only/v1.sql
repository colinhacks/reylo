create schema s; create type s.mood as enum ('a', 'b'); create table t (id integer primary key, a varchar default ('a'::s.mood)::varchar); create table c (id integer references t (id));
