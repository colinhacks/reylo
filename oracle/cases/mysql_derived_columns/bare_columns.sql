select id, n from (select id, (select count(*) from posts) as n from users) d
