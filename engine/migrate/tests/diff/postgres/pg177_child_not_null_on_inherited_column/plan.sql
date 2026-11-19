-- file 1 of 2
alter table c add constraint d_not_null check (d is not null) not valid;
-- file 2 of 2
alter table c validate constraint d_not_null;
alter table c alter column d set not null;
alter table c drop constraint d_not_null;
