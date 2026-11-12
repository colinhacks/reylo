select (select json_object('k', d.t, 'c', count(*)) from posts p group by p.tag order by p.tag limit 1) as o from (select p.title as t from posts p group by p.title with rollup) d
