select id, row_number() over (partition by :p order by :o) as n from users
