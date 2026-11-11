with d as (select user_id from posts group by user_id with rollup) select (select json_object('k', d.user_id, 'c', count(*)) from posts p group by p.tag order by p.tag limit 1) as o from d
