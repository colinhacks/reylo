with d as (select user_id from posts group by rollup (user_id)) select (select jsonb_build_object('k', d.user_id, 'c', count(*)) from posts p group by p.tag order by 1 limit 1) as j from d
