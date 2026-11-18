create type e as enum ('a', 'b');
create table t (id integer primary key, v e[] default '{b}'::e[]);
create type f as enum ('a', 'b');
create table u (id integer primary key, v f default (case when true then 'a' else 'b' end)::f);
