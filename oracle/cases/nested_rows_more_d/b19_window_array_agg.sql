select c.id, array_agg(c) over (order by c.id rows between 1 following and 1 following) as arr from comments c
