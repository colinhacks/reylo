select (select json_object('g', group_concat(p.tag), 'm', max(p.tag), 'gt', group_concat(p.title), 'n', count(*)) from posts p group by p.user_id order by p.user_id limit 1) as o
