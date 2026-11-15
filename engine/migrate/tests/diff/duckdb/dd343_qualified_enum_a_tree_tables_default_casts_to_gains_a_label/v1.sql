create schema b; create type b.mood as enum ('a', 'b'); create table pages (id integer primary key, parent_id integer references pages (id), m varchar default 'a'::b.mood);
