create table t1 (dt date not null, primary key (dt)) partition by range (year(`dt`)) subpartition by hash (dayofmonth(`dt`)) subpartitions 2 (partition p0 values less than (2020) engine = InnoDB, partition p1 values less than maxvalue engine = InnoDB);
create table t2 (`id` int not null, `c` int default null, `g` int generated always as ((`c` + 1)) stored, primary key (`id`));
