create macro b."valid" () as (1);
alter table b.p2 alter column d drop default;
drop macro valid;
alter table b.p2 alter column d set default "valid"();
