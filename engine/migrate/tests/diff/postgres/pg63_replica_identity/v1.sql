create table messages (id integer primary key, body text not null);
create table events (id integer not null, kind text not null);
create unique index events_id on events (id);
