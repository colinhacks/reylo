-- file 1 of 2
alter table t1 add column n0 text default '7';
alter table h0 add column if not exists n0 text default '7' not null;
alter table h0 alter column n0 set default '7'::text;
alter table h0 add constraint n0_not_null check (n0 is not null) not valid;
-- file 2 of 2
alter table h0 validate constraint n0_not_null;
alter table h0 alter column n0 set not null;
alter table h0 drop constraint n0_not_null;
