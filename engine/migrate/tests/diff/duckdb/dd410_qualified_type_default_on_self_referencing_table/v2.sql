create schema s; create type s.mood as enum ('a', 'b', 'c'); create table pages (id integer primary key, parent integer references pages (id), a varchar default ('a'::s.mood)::varchar);
