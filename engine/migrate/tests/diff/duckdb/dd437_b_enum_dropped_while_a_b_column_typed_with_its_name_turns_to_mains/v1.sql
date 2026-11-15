create schema b; create type b.mood as enum ('a', 'b'); create type mood as enum ('a', 'b'); create table b.t (id integer primary key, m mood);
