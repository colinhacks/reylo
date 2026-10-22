select id from (select id from users order by :s::text limit 3) t
