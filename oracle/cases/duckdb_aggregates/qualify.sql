select customer, total from orders qualify row_number() over (partition by customer order by total desc) = 1
