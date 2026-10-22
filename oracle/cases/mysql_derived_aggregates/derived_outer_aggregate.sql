select (select count(u.name) from (select id from posts limit 1) p) as n from users u
