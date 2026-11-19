create table p (a int) partition by range (a);
create table p1 partition of p for values from (0) to (10);
create table p2 partition of p for values from (10) to (20);
create index on p (a);
