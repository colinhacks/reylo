alter table b.p2 alter column d drop default;
drop macro b.wrap;
create macro b.wrap () as (main."valid"('x'));
alter table b.p2 alter column d set default b.wrap();
alter table b.p2 alter column d drop default;
drop macro b.valid;
alter table b.p2 alter column d set default b.wrap();
