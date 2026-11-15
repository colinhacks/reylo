create temporary table reylo_copy_b as select * from b;
drop table b;
create table b (id bigint, author_id bigint, foreign key (author_id) references a (id));
insert into b (id, author_id) select id, a_id from temp.main.reylo_copy_b;
drop table temp.main.reylo_copy_b;
-- b.a_id is renamed author_id, keeping its rows. If they are two columns, drop the one and add the other instead.
