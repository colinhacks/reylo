create function r() returns bigint language sql stable begin atomic select count(n) from t; end;
alter table t alter column n set default r();
