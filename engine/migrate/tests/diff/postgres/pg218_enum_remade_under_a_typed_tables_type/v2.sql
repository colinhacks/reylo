create type e as enum ('a', 'b', 'c');
create type ty as (id int, c e);
create table t of ty;
