select :p::integer as p, count(*) as n from orders group by rollup (1)
