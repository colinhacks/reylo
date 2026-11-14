create table messages (id integer primary key, body text not null);
alter table messages replica identity full;
create table events (id integer not null, kind text not null);
create unique index events_id on events (id);
alter table events replica identity using index events_id;
