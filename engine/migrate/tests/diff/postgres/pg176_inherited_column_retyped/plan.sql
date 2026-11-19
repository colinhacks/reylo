alter table only c alter column c1 drop default;
alter table only p alter column c1 drop default;
alter table p alter column c1 type text;
alter table only p alter column c1 set default '12';
alter table c alter column c1 set default '21'::text;
