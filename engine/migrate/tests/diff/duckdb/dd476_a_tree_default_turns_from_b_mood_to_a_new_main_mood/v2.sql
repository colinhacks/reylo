create schema b; create type mood as enum ('a', 'c'); create table b.pages (id integer primary key, parent_id integer references b.pages (id), m varchar default 'a'::main.mood);
