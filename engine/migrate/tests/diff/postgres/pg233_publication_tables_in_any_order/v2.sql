create table t0 (id int);
create table t1 (id int);
create publication p for table t1, table only t0;
