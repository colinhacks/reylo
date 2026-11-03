create schema api;
create table api.notes (id integer primary key, body text not null);
create function api.note_count() returns bigint language sql stable as 'select count(*) from api.notes';
create function api.purge() returns void language sql as 'delete from api.notes';
