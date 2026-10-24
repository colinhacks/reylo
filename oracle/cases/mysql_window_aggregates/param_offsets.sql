select sum(p.id) over (order by p.id rows between ? preceding and ? following) as around from posts p
