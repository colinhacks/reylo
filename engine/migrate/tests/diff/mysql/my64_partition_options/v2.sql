create table k (id int primary key) partition by key algorithm = 1 (id) partitions 2;
create table r (id int primary key) partition by range (id) (partition p0 values less than (10) comment 'new', partition p1 values less than (20) max_rows = 200);
