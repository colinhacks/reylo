select (select json_object('m', max(e.v), 'a', json_arrayagg(e.v), 'k', concat(e.k, '')) from (select p.tag as k, p.score as v from posts p) e group by e.k order by e.k limit 1) as o
