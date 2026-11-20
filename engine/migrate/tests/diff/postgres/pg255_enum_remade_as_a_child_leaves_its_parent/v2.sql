create type e as enum ('a', 'b');
create table p (id int, c e not null);
create table h (c e not null);
