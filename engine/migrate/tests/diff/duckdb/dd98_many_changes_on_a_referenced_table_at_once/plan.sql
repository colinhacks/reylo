comment on table p is null;
drop index p_name;
create temporary table reylo_copy_p as select * from p;
create temporary table reylo_copy_c as select * from c;
drop table c;
drop table p;
create table p (id integer, title varchar, n bigint, note varchar, primary key (id), check ((n > 0)));
create table c (id integer, p_id integer, extra varchar, foreign key (p_id) references p (id));
insert into p (id, title, n, note) select id, name, n, note from temp.main.reylo_copy_p;
insert into c (id, p_id) select id, p_id from temp.main.reylo_copy_c;
drop table temp.main.reylo_copy_p;
drop table temp.main.reylo_copy_c;
comment on column p.note is 'n';
-- p.name is renamed title, keeping its rows. If they are two columns, drop the one and add the other instead.
create index p_n on p(n);
create index p_title on p(title);
