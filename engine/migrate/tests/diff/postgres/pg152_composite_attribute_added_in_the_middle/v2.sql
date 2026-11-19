create type q as (a integer, b integer, c integer);
create type p as (a integer, b integer, c text);
create table t (id integer primary key, x p default row(1, null, 'z')::p);
