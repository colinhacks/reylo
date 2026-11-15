drop type mood;
create type mood as enum('a', 'b', 'c');
create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t (id integer, code varchar, status enum('a', 'b'), primary key (id), check ("valid"(code)));
insert into t (id, code, status) select id, code, status from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
