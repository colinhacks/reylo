alter type mood rename to mood__old;
create type mood as enum ('a', 'b,c');
alter table t alter column m type mood[] using m::text[]::mood[];
drop type mood__old;
alter table t alter column m set default '{"b,c"}'::mood[];
