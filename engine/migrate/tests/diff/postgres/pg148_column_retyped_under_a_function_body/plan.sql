drop view totals;
drop function total();
alter table t alter column n type bigint;
create function total() returns bigint language sql stable begin atomic select sum(n) from t; end;
create view totals as select total() as total;
