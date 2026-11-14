create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (s, n) as ((length(s) > n));
create table t (id integer, code varchar, primary key (id), check ("valid"(code, 0)));
insert into t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop macro table tm;
create macro tm () as table (select "valid"('x', 0) as ok);
