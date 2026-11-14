create schema b; create type mood as enum ('a', 'b'); create table b.p2 (id integer, d varchar default 'a'::mood);
