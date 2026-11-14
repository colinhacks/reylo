drop type addr;
create type addr as struct(city varchar, zip varchar);
alter table t alter column a set data type addr;
alter table t alter column b set data type struct(city varchar, zip varchar)[];
