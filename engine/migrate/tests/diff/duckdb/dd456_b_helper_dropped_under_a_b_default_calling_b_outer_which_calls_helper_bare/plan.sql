alter table b.t alter column n drop default;
drop macro b.helper;
alter table b.t alter column n set default b."outer"();
