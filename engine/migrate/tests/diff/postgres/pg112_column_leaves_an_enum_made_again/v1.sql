create type e as enum ('a', 'b');
create type f as enum ('a');
create table t (id integer primary key, v e, w e);
