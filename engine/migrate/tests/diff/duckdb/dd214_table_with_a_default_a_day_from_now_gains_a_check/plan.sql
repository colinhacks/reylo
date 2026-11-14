create temporary table reylo_copy_invites as select * from invites;
drop table invites;
create table invites (id integer, email varchar, expires date default (current_date + to_days(cast(trunc(cast(1 as DOUBLE)) as INTEGER))), primary key (id), check ((email != '')));
insert into invites (id, email, expires) select id, email, expires from temp.main.reylo_copy_invites;
drop table temp.main.reylo_copy_invites;
