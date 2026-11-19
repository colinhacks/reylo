create table t (id int primary key, c bigint);
create publication pub for table t (id, c);
