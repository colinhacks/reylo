alter publication pub set table only h;
alter table h no inherit p;
alter table h drop column if exists id;
