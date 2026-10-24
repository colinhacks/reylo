select p.user_id, sum(count(*)) over (order by p.user_id) as running, max(p.user_id) over () as top_user from posts p group by p.user_id with rollup
