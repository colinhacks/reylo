drop index ix;
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, qty integer, n integer, primary key (id), check ((qty > 0)));
insert into t (id, qty) select id, qty from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index ix on t(qty, n);
comment on index ix is 'by qty';
