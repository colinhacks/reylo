-- file 1 of 2
alter table a drop constraint a_b_id_fkey;
alter table a alter column b_id type text;
alter table b alter column id type text;
alter table a add constraint a_b_id_fkey foreign key (b_id) references b(id) not valid;
-- file 2 of 2
alter table a validate constraint a_b_id_fkey;
