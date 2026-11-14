create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro m2;
drop macro m1;
create macro m1 (s) as ((length(s) > 0));
create macro m2 (s) as ((not (s in ('x'))));
create table t (id integer, code varchar, remark varchar, primary key (id), check ((m1(code) and m2(code))));
insert into t (id, code, remark) select id, code, note from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.note is renamed remark, keeping its rows. If they are two columns, drop the one and add the other instead.
