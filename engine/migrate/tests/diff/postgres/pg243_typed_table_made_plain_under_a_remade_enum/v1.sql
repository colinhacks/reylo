create type e as enum ('a', 'b', 'c');
create domain d as e;
create type ty as (id int, n d);
create table t of ty;
