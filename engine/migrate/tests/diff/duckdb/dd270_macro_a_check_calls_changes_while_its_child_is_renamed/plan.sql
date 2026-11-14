-- loses data: c
drop table c;
create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t (id integer, code varchar, primary key (id), check ("valid"(code)));
insert into t (id, code) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create table kids (id integer, t_id integer, note varchar, primary key (id), foreign key (t_id) references t (id));
