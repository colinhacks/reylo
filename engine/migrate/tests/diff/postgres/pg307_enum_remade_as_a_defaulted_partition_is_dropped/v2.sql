create type e as enum ('a', 'c');
create table p (id int, c e default 'a') partition by list (id);
create table k partition of p for values in (2);
