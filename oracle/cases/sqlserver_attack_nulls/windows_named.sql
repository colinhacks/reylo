select sum(n) over w as a from f window w as (order by id rows between 1 following and 1 following)
