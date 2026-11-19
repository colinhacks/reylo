create table p (a int) partition by range (a);
create table p1 partition of p for values from (0) to (10);
create table p2 (a int);
create index on p (a);
create index p2_a_idx on p2 (a);
