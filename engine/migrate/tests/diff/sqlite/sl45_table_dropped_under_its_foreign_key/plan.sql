-- loses data: cat.boxid
-- loses data: box
create table new_Cat ("id" integer not null primary key autoincrement);
insert into new_Cat ("id") select "id" from Cat;
delete from sqlite_sequence where name = 'new_Cat';
insert into sqlite_sequence (name, seq) select 'new_Cat', seq from sqlite_sequence where name = 'Cat';
drop table Cat;
alter table new_Cat rename to Cat;
drop table Box;
