alter table b.pages alter column d drop default;
drop macro b.valid;
create macro b.valid (s) as 'bb';
alter table b.pages alter column d set default memory."valid"('abc');
