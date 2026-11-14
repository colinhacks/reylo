select (select json_object('a' : jsonb_agg(p.id), 'm' : max(p.n), 't' : p.tag) from posts p group by p.tag order by p.tag limit 1) as j
