create type e as enum ('a', 'b', 'x', 'c');
create table p (id int not null, c e) partition by list (id);
create table p0 partition of p for values in (1, 2, 3);
create index i on p (c) where c is not null;
