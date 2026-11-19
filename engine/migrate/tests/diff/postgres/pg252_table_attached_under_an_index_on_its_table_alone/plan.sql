-- loses data: x0.n1
drop index i0;
alter table x0 drop column n1;
alter table x0 rename constraint x0_id_not_null to "t0""x_id_not_null";
alter table "t0""x" attach partition x0 for values from (101) to (104);
create index i0 on only "t0""x" (id);
