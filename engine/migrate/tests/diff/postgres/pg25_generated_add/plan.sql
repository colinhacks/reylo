alter table items add column total numeric generated always as (price * qty) stored;
