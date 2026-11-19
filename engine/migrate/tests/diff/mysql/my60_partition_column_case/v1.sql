create table t (id int primary key) partition by hash (id) partitions 2;
