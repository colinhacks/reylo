-- loses data: p.x
alter table c no inherit p;
alter table c add column y integer;
alter table c inherit q;
alter table c inherit p;
alter table p drop column x;
