alter table k partition by key algorithm = 1 (id) partitions 2;
alter table r partition by range (id) (partition p0 values less than (10) comment = 'new' engine = innodb, partition p1 values less than (20) max_rows = 200 engine = innodb);
