with recursive r (n, role) as (select 1, role from users where id = 1 union all select n + 1, role from r where n < 3) select role from r
