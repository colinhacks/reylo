select max(p.id) over w as ahead from posts p window w as (order by p.id rows between 2 following and unbounded following)
