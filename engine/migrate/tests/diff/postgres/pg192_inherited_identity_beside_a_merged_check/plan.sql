-- file 1 of 2
alter table p drop constraint positive;
alter table p alter column a type integer;
alter table p add constraint positive check (a > 0) not valid;
alter table c alter column a set maxvalue 200;
-- file 2 of 2
alter table p validate constraint positive;
