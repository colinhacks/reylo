alter publication pub drop table only t;
alter table t alter column c type integer;
alter publication pub set table only t (id, c);
