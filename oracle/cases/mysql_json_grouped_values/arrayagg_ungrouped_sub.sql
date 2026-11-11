select (select json_object('a', json_arrayagg(p.score), 'm', max(p.n), 'c', count(*)) from posts p where false) as o
