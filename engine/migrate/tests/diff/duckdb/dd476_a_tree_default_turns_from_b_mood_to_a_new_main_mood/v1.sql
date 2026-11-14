create schema b; create type b.mood as enum ('a'); create table b.pages (id integer primary key, parent_id integer references b.pages (id), m varchar default 'a'::b.mood);
