-- loses data: t_p1
alter table t detach partition t_p1;
drop table t_p1;
