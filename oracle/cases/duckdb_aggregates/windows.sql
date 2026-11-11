select id, row_number() over (order by id) as position, rank() over (partition by region order by total) as rank_in_region,
  sum(total) over (order by id rows between 2 preceding and 1 preceding) as previous_two, lag(customer) over (order by id) as previous_customer
from orders
