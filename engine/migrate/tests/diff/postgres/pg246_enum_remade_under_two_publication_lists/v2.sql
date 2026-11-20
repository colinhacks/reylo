create type e as enum ('a', 'x', 'c');
create table t (id int, c e);
create table h (c e not null);
create publication pub for table only h (c);
