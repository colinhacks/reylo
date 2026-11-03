create or replace function bump(x integer) returns integer language sql immutable as 'select x + 2';
