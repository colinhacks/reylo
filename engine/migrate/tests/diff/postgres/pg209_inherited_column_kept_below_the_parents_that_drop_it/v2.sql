create table a (y int);
create table b () inherits (a);
create table c (x int) inherits (b);
create table p1 ();
create table p2 ();
create table d (z int) inherits (p1, p2);
