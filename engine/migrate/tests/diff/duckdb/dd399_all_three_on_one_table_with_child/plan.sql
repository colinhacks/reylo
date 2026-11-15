-- DuckDB makes seq again, so it starts again at 1: where rows hold its values, start it past them
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
drop type mood;
drop sequence seq;
drop macro m;
create macro m (x) as ((x > -1));
create sequence seq start with 1 increment by 3 minvalue 1 maxvalue 9223372036854775807;
create type mood as enum('a', 'b', 'c');
create table p (id bigint default nextval('seq'), md mood, primary key (id), check (m(id)));
create table c (p bigint, md mood, foreign key (p) references p (id));
insert into p (id, md) select id, md from temp.main.reylo_copy_p;
insert into c (p, md) select p, md from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
