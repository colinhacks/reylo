alter type e rename to e__old;
create type e as enum ('a');
alter table t alter column v drop default;
alter table t alter column v type e[] using v::text[]::e[];
drop type e__old;
alter type f rename to f__old;
create type f as enum ('a');
alter table u alter column v drop default;
alter table u alter column v type f using v::text::f;
alter table u alter column v set default (
case
    when true then 'a'::text
    else 'b'::text
end)::f;
drop type f__old;
alter table t alter column v set default '{a}'::e[];
