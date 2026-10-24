select max(p.id) over (order by p.id rows between :back preceding and :ahead following) as around from posts p
