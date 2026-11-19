-- file 1 of 2
alter table users add constraint phone_not_null check (phone is not null) not valid;
-- file 2 of 2
alter table users validate constraint phone_not_null;
alter table users alter column phone set not null;
alter table users drop constraint phone_not_null;
