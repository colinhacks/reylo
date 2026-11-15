alter table b.p2 alter column d drop default;
drop macro b.valid;
alter table b.p2 alter column d set default "valid"('abc');
