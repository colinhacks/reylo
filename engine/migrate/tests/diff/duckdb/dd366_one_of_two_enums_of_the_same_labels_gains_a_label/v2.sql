create type mood as enum ('a', 'b', 'c'); create type feel as enum ('a', 'b'); create table t (id integer primary key, m mood, f feel);
