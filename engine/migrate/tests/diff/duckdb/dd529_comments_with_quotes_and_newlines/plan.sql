create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, a varchar, primary key (id), check ((a != '')));
insert into t (id, a) select id, a from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
comment on column t.a is 'a "quoted" one';
comment on table t is 'it''s
multi; line';
