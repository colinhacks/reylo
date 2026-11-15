create temporary table reylo_copy_p2 as select * from b.p2;
drop table b.p2;
drop macro k;
create macro k (x) as ((x > -1));
create table b.p2 (id integer, n integer, e integer default "valid"(), check (k(n)));
insert into b.p2 (id, n, e) select id, n, d from temp.main.reylo_copy_p2;
drop table temp.main.reylo_copy_p2;
-- b.p2.d is renamed e, keeping its rows. If they are two columns, drop the one and add the other instead.
alter table b.p2 alter column e drop default;
drop macro b.valid;
alter table b.p2 alter column e set default "valid"();
