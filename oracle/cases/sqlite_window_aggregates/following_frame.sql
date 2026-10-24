select max(p.id) over (order by p.id rows between 1 following and 2 following) as ahead from posts p
