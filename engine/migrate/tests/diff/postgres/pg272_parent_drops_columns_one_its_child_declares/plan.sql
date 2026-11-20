-- loses data: t.e
-- loses data: t.n
-- loses data: h.n
alter table t drop column e;
alter table t drop column n;
drop type p;
alter table h alter column e drop default;
