comment on column t.note is null;
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, remark varchar, n integer, primary key (id), check ((n > 0)));
insert into t (id, remark, n) select id, note, n from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.note is renamed remark, keeping its rows. If they are two columns, drop the one and add the other instead.
comment on column t.remark is 'free text';
