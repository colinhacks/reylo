create extension postgres_fdw;
create extension citext;
create server remote foreign data wrapper postgres_fdw;
create foreign table ft (a int options (column_name 'b')) server remote;
create function f() returns int language sql as 'select 1';
alter function f() depends on extension citext;
create domain code as text collate "C";
comment on schema public is 'the project''s';
