create table p (id int) partition by range (id);
create table p1 partition of p for values from (0) to (10);
create publication everything for all tables;
create publication root for table p;
