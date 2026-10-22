select id, row_number() over (partition by :p::text order by :o::text) as n from users
