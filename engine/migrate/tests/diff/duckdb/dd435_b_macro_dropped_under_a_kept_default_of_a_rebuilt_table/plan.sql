create temporary table reylo_copy_p2 as select * from b.p2;
drop table b.p2;
create table b.p2 (id integer, n integer, d varchar default "valid"('abc'), primary key (id), check ((n > 0)));
insert into b.p2 (id, n, d) select id, n, d from temp.main.reylo_copy_p2;
drop table temp.main.reylo_copy_p2;
alter table b.p2 alter column d drop default;
drop macro b.valid;
alter table b.p2 alter column d set default "valid"('abc');
