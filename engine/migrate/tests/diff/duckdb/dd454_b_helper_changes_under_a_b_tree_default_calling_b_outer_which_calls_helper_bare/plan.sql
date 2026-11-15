alter table b.pages alter column n drop default;
drop macro b.helper;
create macro b.helper () as 20;
alter table b.pages alter column n set default b."outer"();
