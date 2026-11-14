create table new_notes (id integer primary key autoincrement, body text not null);
insert into new_notes (id, body) select id, body from notes;
delete from sqlite_sequence where name = 'new_notes';
insert into sqlite_sequence (name, seq) select 'new_notes', seq from sqlite_sequence where name = 'notes';
drop table notes;
alter table new_notes rename to notes;
