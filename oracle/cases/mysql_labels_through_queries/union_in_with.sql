with w (r) as (select role from users union all select role from users) select r from w
