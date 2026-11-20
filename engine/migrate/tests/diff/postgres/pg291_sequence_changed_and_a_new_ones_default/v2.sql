create sequence seq1 increment by 1;
create sequence seq2;
create table t1 (c1 int);
create table t2 (c3 int, c4 int default nextval('seq2')) inherits (t1);
