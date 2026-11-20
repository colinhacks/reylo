set check_function_bodies = off;
drop function score(mood);
alter type mood rename to mood__old;
create type mood as enum ('happy', 'ok', 'sad');
alter table t alter column m type mood using m::text::mood;
drop type mood__old;
create function score(m mood) returns integer language sql immutable as 'select 1';
reset check_function_bodies;
