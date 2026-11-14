with s as (select p.tag, p.score from posts p) select (select json_object('m', max(s.score), 'a', json_arrayagg(s.score), 'k', concat(s.tag, '')) from s group by s.tag order by s.tag limit 1) as o
