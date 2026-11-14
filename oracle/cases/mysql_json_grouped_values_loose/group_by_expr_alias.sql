select (select json_object('k', lower(p.title), 'c', count(*), 'a', json_arrayagg(p.score)) from posts p group by lower(p.title) order by lower(p.title) limit 1) as o
