create table new_events (name text not null, at text not null);
insert into new_events (rowid, name, at) select rowid, name, at from events;
drop table events;
alter table new_events rename to events;
