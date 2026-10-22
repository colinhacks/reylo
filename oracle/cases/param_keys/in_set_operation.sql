(select id from users order by :s::text limit 2) union all select id from users
