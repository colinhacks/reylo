select sum(p.id) over (order by p.id rows between unbounded preceding and 1 preceding) as before_row from posts p
