create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, s varchar, n integer, primary key (id), check (((cast(struct_pack(a := s) as STRUCT(a VARCHAR))).a is not null)), check ((n > 0)));
insert into t (id, s) select id, s from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
