create type e as enum ('a', 'b', 'c');
create table p (id int, c e default 'a') partition by list (id);
create table h partition of p for values in (1);
create table k partition of p for values in (2);
