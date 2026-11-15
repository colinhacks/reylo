drop view rpt.Totals;
create view rpt.Totals as select id, amount as "Total Sum", (amount * 2) as twice from rpt.sales;
comment on column plain."select" is 'changed';
comment on column rpt.Totals."Total Sum" is 'all of it';
comment on view rpt.Totals is 'v';
