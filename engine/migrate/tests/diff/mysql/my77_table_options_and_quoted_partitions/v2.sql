create table t1 (id int primary key) row_format=compressed;
create table regional_sales (id bigint not null auto_increment, region_code int not null, primary key (id, region_code)) partition by list (region_code) (partition `p north` values in (1, 2, 3), partition `p south` values in (4, 5, 6), partition `p east` values in (7, 8, 9));
