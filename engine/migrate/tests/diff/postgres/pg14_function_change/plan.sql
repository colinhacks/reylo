set check_function_bodies = off;
create or replace function bump(x integer) returns integer language sql immutable as 'select x + 2';
reset check_function_bodies;
