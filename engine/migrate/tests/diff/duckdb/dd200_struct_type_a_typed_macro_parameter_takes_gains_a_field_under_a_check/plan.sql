create temporary table reylo_copy_t as select * from t;
drop table t;
drop type rec;
drop macro positive;
create macro positive (r struct(n integer, m integer)) as ((r.n > 0));
create type rec as struct(n integer, m integer);
create table t (id integer, r rec, primary key (id), check (positive(r)));
insert into t (id, r) select id, r from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
