create table p (id int, x int, y int);
create table c () inherits (p);
create table q (id int, x int);
create table d () inherits (q);
