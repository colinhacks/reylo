create table a (id int, c bigint);
create table b (id int, c bigint not null);
create table h () inherits (a, b);
