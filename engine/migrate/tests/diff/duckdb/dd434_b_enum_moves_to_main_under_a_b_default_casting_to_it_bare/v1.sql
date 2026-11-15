create schema b; create type b.mood as enum ('a', 'b'); create table b.p2 (id integer primary key, m varchar default 'a'::mood);
