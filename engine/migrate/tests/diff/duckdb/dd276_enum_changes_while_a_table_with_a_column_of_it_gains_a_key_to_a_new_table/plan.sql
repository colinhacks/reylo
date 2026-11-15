drop type mood;
create type mood as enum('a', 'b', 'c');
create table p (id integer, primary key (id));
create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, m mood, p integer, primary key (id), foreign key (p) references p (id));
insert into t (id, m, p) select id, m, p from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
