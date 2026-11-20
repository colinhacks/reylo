-- loses data: v1
drop table v1;
alter table c add constraint fk_self foreign key (parent_id) references c (id);
create view v1 as select t.id as id from t;
