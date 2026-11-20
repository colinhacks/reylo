create table t (id int primary key) partition by hash (ID) partitions 2;
