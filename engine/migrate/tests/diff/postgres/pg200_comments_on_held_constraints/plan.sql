-- file 1 of 3
alter table x drop constraint x_c_key;
alter domain d2 drop constraint d2_check;
alter domain d2 add constraint d2_check check (value > 1);
alter table x add column d integer;
-- file 2 of 3
create unique index concurrently x_c_key on x (c, d);
-- file 3 of 3
alter table x add constraint x_c_key unique using index x_c_key;
comment on constraint d1_check on domain d1 is 'positive';
comment on constraint d2_check on domain d2 is 'positive';
comment on constraint t_a_not_null on t is 'never missing';
comment on index t_pkey is 'the key''s index';
comment on index x_c_key is 'the index of a key';
comment on statistics s is 'a with c';
