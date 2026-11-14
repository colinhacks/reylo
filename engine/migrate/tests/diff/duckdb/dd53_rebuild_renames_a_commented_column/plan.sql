comment on column t.nick is null;
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id bigint, handle varchar, primary key (id), check ((handle != '')));
insert into t (id, handle) select id, nick from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.nick is renamed handle, keeping its rows. If they are two columns, drop the one and add the other instead.
comment on column t.handle is 'what they go by';
