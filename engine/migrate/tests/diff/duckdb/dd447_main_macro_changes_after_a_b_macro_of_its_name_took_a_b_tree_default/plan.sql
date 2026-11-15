alter table b.pages alter column d drop default;
drop macro valid;
create macro valid (s) as 'main2';
alter table b.pages alter column d set default "valid"('abc');
