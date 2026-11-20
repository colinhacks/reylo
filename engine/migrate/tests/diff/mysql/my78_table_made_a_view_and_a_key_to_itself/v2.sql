create table t (id int);
create view v1 as select * from t;
create table c (id int primary key, parent_id int, constraint fk_self foreign key (parent_id) references c (id));
