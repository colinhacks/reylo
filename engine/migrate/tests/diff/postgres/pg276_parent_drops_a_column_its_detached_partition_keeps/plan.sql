-- loses data: t.n
-- loses data: t0.n
alter table t detach partition t1;
alter table t1 rename constraint t_id_not_null to t1_id_not_null;
alter table t drop column n;
