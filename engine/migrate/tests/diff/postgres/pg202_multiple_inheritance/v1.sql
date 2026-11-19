create table p1 (a int);
create table p2 (b int default 2, c int);
create table c (x int) inherits (p1);
create table d (x int) inherits (p1, p2);
