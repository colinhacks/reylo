alter table c add column b integer default 2;
alter table c add column c integer;
alter table c inherit p2;
alter table d no inherit p2;
alter table d drop column if exists b;
alter table d drop column if exists c;
create table n (
  y integer
) inherits (p2, p1);
