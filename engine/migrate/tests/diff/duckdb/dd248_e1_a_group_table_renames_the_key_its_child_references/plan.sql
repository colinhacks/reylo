create temporary table reylo_copy_t as select * from t;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table t;
drop macro x;
create macro x () as (2);
create table t (tid integer, v integer, primary key (tid), check ((v >= x())));
create table c (id integer, t_id integer, primary key (id), foreign key (t_id) references t (tid));
insert into t (tid, v) select id, v from temp.main.reylo_copy_t;
insert into c (id, t_id) select id, t_id from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_c;
-- t.id is renamed tid, keeping its rows. If they are two columns, drop the one and add the other instead.
