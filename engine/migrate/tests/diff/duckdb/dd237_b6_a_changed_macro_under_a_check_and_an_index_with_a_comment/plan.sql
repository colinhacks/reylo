create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro x;
create macro x (v) as ((v + 2));
create table t (id integer, v integer, primary key (id), check ((x(v) > 0)));
insert into t (id, v) select id, v from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create index t_x on t((x(v)));
comment on index t_x is 'by x';
