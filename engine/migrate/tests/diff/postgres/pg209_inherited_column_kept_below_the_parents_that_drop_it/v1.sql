create table a (x int, y int);
create table b () inherits (a);
create table c () inherits (b);
create table p1 (z int);
create table p2 (z int);
create table d () inherits (p1, p2);
