create type e as enum ('a');
create type f as enum ('a');
create table t (id integer primary key, v f, w e);
