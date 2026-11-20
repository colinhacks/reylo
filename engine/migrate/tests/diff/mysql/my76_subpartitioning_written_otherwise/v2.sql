create table t1 (dt date not null, primary key (dt)) partition by range (YEAR(dt)) subpartition by hash (dayofmonth(dt)) subpartitions 2 (partition p0 values less than (2020), partition p1 values less than maxvalue);
create table t2 (id int primary key, c int, g int as (C + 1) stored);
