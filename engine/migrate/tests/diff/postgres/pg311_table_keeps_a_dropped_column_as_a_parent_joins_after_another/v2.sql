create table a (id int);
create table q (y int);
create table b (z int);
create table c (x int) inherits (a, q, b);
create table a2 (id int);
create table q2 (y int);
create table d (x int not null) inherits (a2, q2);
