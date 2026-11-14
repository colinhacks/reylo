create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, "check" varchar, "foreign" integer, primary key (id), unique ("check", "foreign"));
insert into t (id, "check", "foreign") select id, "check", "foreign" from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
