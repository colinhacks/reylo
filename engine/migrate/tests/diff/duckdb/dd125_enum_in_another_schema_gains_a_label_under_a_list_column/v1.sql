create schema s; create type s.mood as enum ('a', 'b'); create table s.t (id integer primary key, ms s.mood[], m s.mood);
