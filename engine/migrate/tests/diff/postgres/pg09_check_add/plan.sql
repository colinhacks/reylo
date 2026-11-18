-- file 1 of 2
alter table users add constraint n_pos check (n > 0) not valid;
-- file 2 of 2
alter table users validate constraint n_pos;
