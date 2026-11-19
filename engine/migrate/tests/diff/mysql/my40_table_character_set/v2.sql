create table kept (id int primary key);
create table legacy (id int primary key, name varchar(10)) default charset=utf8mb4 collate=utf8mb4_bin;
create table test_trigger (id int not null auto_increment, sort_order int not null, primary key (id)) engine=InnoDB default charset=utf8;
