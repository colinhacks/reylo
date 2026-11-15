alter table b.t alter column d drop default;
drop macro valid;
alter table b.t alter column d set default "valid"('abc');
