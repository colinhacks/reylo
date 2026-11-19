create table readings (
  id int primary key,
  level int,
  code varchar(10),
  kind enum('low', 'mid', 'high'),
  ratio decimal(6,2)
);
