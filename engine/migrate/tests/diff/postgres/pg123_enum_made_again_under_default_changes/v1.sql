create type e as enum ('a', 'b');
create table t (id integer primary key, v e default 'a');
create type g as enum ('a', 'b');
create table u (id integer primary key, v g default 'a');
