select (select json_object('c', count(distinct p.tag), 'g', group_concat(distinct p.title), 'm', max(distinct p.score)) from posts p group by p.user_id order by p.user_id limit 1) as o
