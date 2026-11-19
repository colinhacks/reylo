-- loses data: a.x
-- loses data: b.x
-- loses data: p1.z
-- loses data: p2.z
alter table c no inherit b;
alter table c inherit b;
alter table a drop column x;
alter table d no inherit p1;
alter table d no inherit p2;
alter table d inherit p1;
alter table d inherit p2;
alter table p1 drop column z;
alter table p2 drop column z;
