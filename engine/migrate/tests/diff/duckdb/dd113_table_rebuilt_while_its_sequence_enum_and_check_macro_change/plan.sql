drop type mood;
create type mood as enum('a', 'b', 'c');
-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_t as select * from t;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table t;
drop sequence s;
drop macro valid;
create macro "valid" (c) as ((length(c) > 0));
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
create table t (id bigint default nextval('s'), m mood, code varchar, n integer, primary key (id), check ("valid"(code)), check ((n > 0)));
create table c (id integer, t_id bigint, primary key (id), foreign key (t_id) references t (id));
insert into t (id, m, code) select id, m, code from temp.main.reylo_copy_t;
insert into c (id, t_id) select id, t_id from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_c;
create index t_m on t(m);
comment on column t.code is 'the code';
