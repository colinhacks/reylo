create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t (id integer, code varchar, primary key (id), check ("valid"(code)));
insert into t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create table u (id integer, name varchar, primary key (id), check ("valid"("name")));
