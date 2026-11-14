create temporary table reylo_copy_t as select * from t;
drop table t;
drop macro valid;
create macro "valid" (s) as ((length(s) > 0));
create table t (id integer, label varchar, primary key (id), check ("valid"("label")));
insert into t (id, label) select id, code from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
create table c (id integer, t_id integer, primary key (id), foreign key (t_id) references t (id));
-- t.code is renamed label, keeping its rows. If they are two columns, drop the one and add the other instead.
