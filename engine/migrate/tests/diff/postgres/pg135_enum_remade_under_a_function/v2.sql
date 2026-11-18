create type mood as enum ('happy', 'ok', 'sad');
create function score(m mood) returns integer language sql immutable as 'select 1';
create table t (id integer primary key, m mood);
