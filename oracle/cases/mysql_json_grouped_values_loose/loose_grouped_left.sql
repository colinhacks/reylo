select json_object('t', p.title, 'x', concat(p.title, ''), 'n', count(*), 'a', json_arrayagg(p.id)) as o from users u left join posts p on p.user_id = u.id group by u.id
