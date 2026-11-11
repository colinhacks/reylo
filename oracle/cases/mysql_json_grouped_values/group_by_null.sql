select (select json_object('c', count(*), 'a', json_arrayagg(p.id)) from posts p group by null) as o
