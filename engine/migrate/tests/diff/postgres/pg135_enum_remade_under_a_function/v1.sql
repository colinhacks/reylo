create type mood as enum ('sad', 'ok', 'happy');
create function score(m mood) returns integer language sql immutable as 'select 1';
create table t (id integer primary key, m mood);
