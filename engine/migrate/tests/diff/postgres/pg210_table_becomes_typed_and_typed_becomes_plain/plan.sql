create type ty as (
  a integer,
  b text
);
alter table t add column b text;
alter table t of ty;
alter table t2 not of;
alter table t2 drop column if exists b;
