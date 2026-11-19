create type e as enum ('a', 'c');
create domain d as e;
create table t (id int, n d);
