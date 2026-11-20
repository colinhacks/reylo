-- loses data: p.x
-- loses data: p.y
-- loses data: q.x
alter table c no inherit p;
alter table p drop column x;
alter table p drop column y;
alter table d no inherit q;
alter table q drop column x;
alter table c inherit p;
