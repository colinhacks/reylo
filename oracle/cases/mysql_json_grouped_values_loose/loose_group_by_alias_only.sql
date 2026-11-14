select json_object('n', count(*), 'x', concat(p.title, '')) as o, p.user_id as uid from posts p group by uid
