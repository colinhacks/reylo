create type e as enum ('a', 'b', 'c', 'd');
create table t (id integer primary key, category e);
create table created_with_e (id integer, category e);
