create sequence new_s start with 100 increment by 1 minvalue 1 maxvalue 9223372036854775807;
create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (c) as ((length(c) > 0));
create table t (id bigint default nextval('new_s'), code varchar, primary key (id), check ("valid"(code)));
insert into t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop sequence old_s;
