create type ty as (
  id integer,
  a integer,
  n integer
);
alter table t add column n integer default 5;
alter table t of ty;
