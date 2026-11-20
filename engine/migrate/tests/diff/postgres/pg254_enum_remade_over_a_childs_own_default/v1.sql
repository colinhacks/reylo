create type e as enum ('a', 'b', 'c', 'x');
create table p (id int, c e);
create table h (c e default 'a') inherits (p);
