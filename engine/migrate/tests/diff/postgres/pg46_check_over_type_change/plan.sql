alter table accounts drop constraint accounts_tier_check;
alter table accounts alter column tier type character varying(20);
alter table accounts add constraint accounts_tier_check check (tier in ('free', 'pro'));
