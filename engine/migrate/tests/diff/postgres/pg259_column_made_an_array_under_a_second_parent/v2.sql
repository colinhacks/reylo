create type e as enum ('a', 'b');
create table a (id int, parent int);
create table b (id int, c e[]);
create table h () inherits (a, b);
