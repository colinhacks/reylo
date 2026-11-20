alter table regional_sales partition by list (region_code) (partition `p north` values in (1, 2, 3) engine = innodb, partition `p south` values in (4, 5, 6) engine = innodb, partition `p east` values in (7, 8, 9) engine = innodb);
alter table t1 row_format=COMPRESSED;
