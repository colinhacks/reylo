create table a (id int not null);
create table b (id int not null);
create table h (id int) inherits (a, b);
