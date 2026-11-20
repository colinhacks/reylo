create extension postgres_fdw;
create extension citext;
create server remote foreign data wrapper postgres_fdw;
create foreign table ft (a int) server remote;
create function f() returns int language sql as 'select 1';
create domain code as text;
