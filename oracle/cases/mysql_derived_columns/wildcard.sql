select * from (select id, name, email, (select count(*) from posts) as n from users) d
