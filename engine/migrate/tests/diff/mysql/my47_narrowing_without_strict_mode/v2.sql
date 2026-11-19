create table readings (
  id int primary key,
  level tinyint,
  code varchar(3),
  kind enum('low', 'mid'),
  ratio decimal(4,2)
);
