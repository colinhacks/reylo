create table hits (id int primary key, n int, m int) partition by hash (id) partitions 4;
