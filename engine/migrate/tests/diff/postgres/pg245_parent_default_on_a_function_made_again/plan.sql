set check_function_bodies = off;
alter table h alter column c drop default;
alter table only p alter column c drop default;
drop function f();
create function f(a text) returns text language sql immutable return '8';
alter table h alter column c set default (f('7'::text))::smallint;
alter table only p alter column c set default f('7')::smallint;
reset check_function_bodies;
