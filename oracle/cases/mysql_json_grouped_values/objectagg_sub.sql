select (select json_object('o', json_objectagg(p.title, p.score), 'n', count(*)) from posts p group by p.user_id order by p.user_id limit 1) as o
