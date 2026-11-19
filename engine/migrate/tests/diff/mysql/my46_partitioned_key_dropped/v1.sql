create table t (id int not null, d date not null, primary key (id, d)) partition by range (year(d)) (partition p0 values less than (2020), partition p1 values less than maxvalue);
