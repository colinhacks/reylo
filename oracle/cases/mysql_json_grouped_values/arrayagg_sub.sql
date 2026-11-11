select (select json_object('a', json_arrayagg(p.tag), 'n', count(*), 'm', max(p.score)) from posts p group by p.user_id order by p.user_id limit 1) as o
