alter table b.pages alter column d drop default;
drop macro valid;
create macro valid () as 3;
alter table b.pages alter column d set default "valid"();
