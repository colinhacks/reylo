create type e as enum ('a', 'b', 'c');
create table t (id integer primary key, category e);
