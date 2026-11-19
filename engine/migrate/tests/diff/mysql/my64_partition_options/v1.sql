create table k (id int primary key) partition by key (id) partitions 2;
create table r (id int primary key) partition by range (id) (partition p0 values less than (10) comment 'old', partition p1 values less than (20) max_rows = 100);
