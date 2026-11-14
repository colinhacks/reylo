create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, "primary" varchar, "unique" varchar, note varchar, unique ("unique"), check ((note != 'primary key, unique')), primary key (id, "primary"));
insert into t (id, "primary", "unique", note) select id, "primary", "unique", note from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
