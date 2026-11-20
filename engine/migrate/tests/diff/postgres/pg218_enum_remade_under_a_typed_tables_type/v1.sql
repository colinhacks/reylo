create type e as enum ('a', 'b', 'c', 'x');
create type ty as (id int, c e);
create table t of ty;
