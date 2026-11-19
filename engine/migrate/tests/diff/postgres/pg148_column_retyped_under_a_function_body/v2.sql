create table t (id integer primary key, n bigint);
create function total() returns bigint language sql stable begin atomic select sum(n) from t; end;
create view totals as select total() as total;
