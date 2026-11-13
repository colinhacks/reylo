select (select json_object('m', max(posts.id), 'c', count(*), 'a', json_arrayagg(p.id)) from posts p group by p.title order by p.title limit 1) as o from posts
