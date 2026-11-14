with recursive r(n, m) as (select 1, 1 union all select n + 1, null::int from r where n < 3) select array(select r from r) as arr
