with posts as (select id from posts) select (with recursive r (n) as (select 1 union all select n + 1 from r where n < 3) select count(n) from r, posts) as c
