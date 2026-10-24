select sum(p.id) over w2 as total from posts p window w1 as (partition by p.user_id), w2 as (w1 order by p.id range between unbounded preceding and unbounded following)
