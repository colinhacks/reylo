create table t (id int not null);
create table h () inherits (t);
create table g (id int) inherits (h);
