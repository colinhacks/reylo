create table t1 (id int not null, d datetime not null, primary key (id, d)) partition by range columns (d) (partition p0 values less than ('2026-11-01'));
