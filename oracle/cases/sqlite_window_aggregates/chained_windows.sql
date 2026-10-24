select max(p.id) over (w1 order by p.id range between unbounded preceding and unbounded following) as top from posts p window w1 as (partition by p.user_id)
