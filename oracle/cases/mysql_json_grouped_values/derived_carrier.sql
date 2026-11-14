select d.o from (select (select json_object('a', json_arrayagg(p.id), 'm', max(p.n), 'k', concat(p.user_id, '')) from posts p group by p.user_id order by p.user_id limit 1) as o) d
