select u.id, array(select c from (select p.id + 1 as n, p.title as t, upper(p.title) as up) c) as arr from users u left join posts p on p.author_id = u.id and false
