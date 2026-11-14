create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro f;
create macro f (x) as ((x != 'b'));
create table t (id integer, m varchar, primary key (id), check (f(m)));
insert into t (id, m) select id, m from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
drop type mood;
