select sum(p.id) over (order by p.id) as running from posts p
