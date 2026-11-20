create table t (id int primary key, c int);
create publication pub for table t (id, c);
