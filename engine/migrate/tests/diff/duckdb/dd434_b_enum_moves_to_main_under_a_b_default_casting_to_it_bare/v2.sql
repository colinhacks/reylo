create schema b; create type mood as enum ('a', 'c'); create table b.p2 (id integer primary key, m varchar default 'a'::mood);
