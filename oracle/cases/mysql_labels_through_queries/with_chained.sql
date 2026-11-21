with a as (select role from users), b as (select role as r from a) select r from b
