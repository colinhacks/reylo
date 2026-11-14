create schema b; create type mood as enum ('a', 'b'); create type b.mood as enum ('a', 'c'); create table b.p2 (id integer, d varchar default 'a'::mood);
