-- loses data: cat.ownerid
-- loses data: other
drop index cat_ownerid_idx;
drop table other;
create table new_cat (id integer primary key autoincrement);
insert into new_cat (id) select id from cat;
delete from sqlite_sequence where name = 'new_cat';
insert into sqlite_sequence (name, seq) select 'new_cat', seq from sqlite_sequence where name = 'cat';
drop table cat;
alter table new_cat rename to cat;
-- ownerid is not null with no default, so the copy fails on a table that has rows: give it a default, or add it nullable, fill it, then make it not null
create table new_owner (id integer primary key autoincrement, ownerid integer not null, constraint owner_ownerid_fkey foreign key (ownerid) references cat (id) on delete restrict on update cascade);
insert into new_owner (id) select id from owner;
delete from sqlite_sequence where name = 'new_owner';
insert into sqlite_sequence (name, seq) select 'new_owner', seq from sqlite_sequence where name = 'owner';
drop table owner;
alter table new_owner rename to owner;
create index ownerid on owner (ownerid);
