create table t (id integer primary key, c numeric);
create function r() returns text language sql stable begin atomic select min((c)::text) from t; end;
