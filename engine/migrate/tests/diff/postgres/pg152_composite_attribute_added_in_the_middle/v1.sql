create type q as (a integer, c integer);
create type p as (a integer, c text);
create table t (id integer primary key, x p default row(1, 'z')::p);
