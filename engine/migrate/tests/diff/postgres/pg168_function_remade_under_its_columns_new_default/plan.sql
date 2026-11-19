drop function r();
alter table t alter column c type text;
create function r() returns text language sql stable begin atomic select min((c)::text) from t; end;
alter table t alter column c set default r();
