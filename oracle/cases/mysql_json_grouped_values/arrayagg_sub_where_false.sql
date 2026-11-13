select (select json_object('a', json_arrayagg(p.score), 'c', count(*)) from posts p where false group by p.user_id) as o
