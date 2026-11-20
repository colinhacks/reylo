-- loses data: t1.d changes from datetime to date, which keeps only the date
alter table t1 remove partitioning;
alter table t1 modify column d date not null;
alter table t1 partition by range columns(d) (partition p0 values less than ('2026-11-01') engine = innodb, partition p1 values less than ('2026-12-01') engine = innodb);
