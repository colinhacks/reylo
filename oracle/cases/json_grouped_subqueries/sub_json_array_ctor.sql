select (select json_array(jsonb_agg(p.id), max(p.n), p.tag null on null) from posts p group by p.tag order by p.tag limit 1) as j
