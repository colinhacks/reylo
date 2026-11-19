-- file 1 of 2
alter table accounts drop constraint accounts_tier_check;
alter table accounts alter column tier type character varying(20);
alter table accounts add constraint accounts_tier_check check (tier in ('free', 'pro')) not valid;
-- file 2 of 2
alter table accounts validate constraint accounts_tier_check;
