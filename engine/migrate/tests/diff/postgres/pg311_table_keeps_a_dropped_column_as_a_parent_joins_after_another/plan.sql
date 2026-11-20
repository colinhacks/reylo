-- loses data: a2.x
-- loses data: a.x
alter table c no inherit a;
alter table d no inherit a2;
alter table a2 drop column x;
create table q (
  y integer
);
alter table c no inherit b;
alter table c add column y integer;
alter table c inherit q;
alter table c inherit b;
alter table a drop column x;
create table q2 (
  y integer
);
alter table d rename constraint a2_x_not_null to d_x_not_null;
alter table d add column y integer;
alter table d inherit q2;
alter table c inherit a;
alter table c no inherit q;
alter table c inherit q;
alter table c no inherit b;
alter table c inherit b;
alter table d inherit a2;
alter table d no inherit q2;
alter table d inherit q2;
