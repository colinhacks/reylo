create temporary table reylo_copy_t as select * from t;
drop table t;
drop type mood;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create type mood as enum('a', 'b', 'c');
create table t (id integer, code varchar, remark varchar, primary key (id), check (("valid"(code) and (cast(code as mood) is not null))));
insert into t (id, code, remark) select id, code, note from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
-- t.note is renamed remark, keeping its rows. If they are two columns, drop the one and add the other instead.
