alter publication pub drop table only p;
alter publication pub drop table only h;
alter table p alter column c type text;
alter publication pub set table only h (id, c), table only p (id, c);
