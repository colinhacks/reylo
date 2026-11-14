create macro "valid" (s) as ('main');
alter table b.p2 alter column d set default main."valid"('abc');
drop macro b.valid;
