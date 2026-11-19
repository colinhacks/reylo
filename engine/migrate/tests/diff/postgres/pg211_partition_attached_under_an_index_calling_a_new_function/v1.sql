create table p (a int) partition by range (a);
create table child (a int);
