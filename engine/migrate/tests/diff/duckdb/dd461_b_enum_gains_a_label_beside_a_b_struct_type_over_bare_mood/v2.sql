create schema b; create type mood as enum ('m'); create type b.mood as enum ('b', 'c'); create type b.pair as struct(x mood); create table b.t (id integer primary key, p b.pair);
