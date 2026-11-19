create type ty as (id int, n bigint);
create table t of ty;
create function r() returns bigint language sql stable begin atomic select count(id) from t; end;
alter table t alter column n set default r();
