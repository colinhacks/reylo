drop type addr;
create type addr as struct(city varchar, zip varchar);
create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t (id integer, code varchar, other struct(city varchar), primary key (id), check ("valid"(code)));
insert into t (id, code, other) select id, code, other from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
