-- file 1 of 2
alter table c alter column b set expression as (a + 2);
alter table c add constraint b_not_null check (b is not null) not valid;
-- file 2 of 2
alter table c validate constraint b_not_null;
alter table c alter column b set not null;
alter table c drop constraint b_not_null;
