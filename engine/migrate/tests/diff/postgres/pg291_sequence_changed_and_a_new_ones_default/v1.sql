create sequence seq1 increment by 2;
create table t1 (c1 int);
create table t2 (c3 int) inherits (t1);
