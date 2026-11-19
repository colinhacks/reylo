-- loses data: d.b
-- loses data: d.c
alter table d no inherit p2;
alter table d drop column if exists b;
alter table d drop column if exists c;
create table n (
  y integer
) inherits (p2, p1);
alter table c add column if not exists b integer default 2;
alter table c add column if not exists c integer;
alter table c inherit p2;
