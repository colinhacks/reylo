create table a (id int, x int);
create table b (z int);
create table c () inherits (a, b);
create table a2 (id int, x int not null);
create table d () inherits (a2);
