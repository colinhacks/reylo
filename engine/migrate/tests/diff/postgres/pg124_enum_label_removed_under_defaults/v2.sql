create type e as enum ('a');
create table t (id integer primary key, v e[] default '{a}'::e[]);
create type f as enum ('a');
create table u (id integer primary key, v f default (case when true then 'a' else 'b' end)::f);
