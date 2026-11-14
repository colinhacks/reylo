create schema b; create type mood as enum ('a', 'b', 'z'); create table b.p2 (id integer, d varchar default 'a'::mood);
