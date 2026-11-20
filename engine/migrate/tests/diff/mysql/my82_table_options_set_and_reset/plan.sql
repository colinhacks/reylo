create table hashed (
  id int
)
partition by key algorithm = 1 (id) partitions 2;
create table invoices (
  id int not null auto_increment,
  total int,
  primary key (id)
) auto_increment=1000;
alter table linked connection='';
alter table merged
  insert_method=FIRST,
  union=(`a`,`b`);
alter table placed tablespace=innodb_file_per_table;
alter table texts drop index ft;
alter table texts add fulltext key ft (t);
