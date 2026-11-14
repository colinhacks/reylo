select distinct min(1) over (order by id rows between 1 preceding and 1 preceding) as m from users
