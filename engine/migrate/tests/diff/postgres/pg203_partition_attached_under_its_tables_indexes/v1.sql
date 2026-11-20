create table p (a int, b int) partition by range (a);
create index p_a on p (a);
create table p1 (a int, b int);
create table p2 (a int, b int);
create index p2_kept on p2 (a);
create table q (a int) partition by range (a);
create index qi on only q (a);
create table q1 (a int);
