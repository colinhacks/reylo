alter table legacy
  modify column name varchar(10) collate utf8mb4_bin,
  collate=utf8mb4_bin;
create table test_trigger (
  id int not null auto_increment,
  sort_order int not null,
  primary key (id)
) collate=utf8mb3_general_ci;
