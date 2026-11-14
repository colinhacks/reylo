select (select json_object('m', max(p.n), 'c', count(*), 'a', json_arrayagg(p.id)) from posts p join posts q on q.user_id = u.id and q.id = p.id) as o from users u
