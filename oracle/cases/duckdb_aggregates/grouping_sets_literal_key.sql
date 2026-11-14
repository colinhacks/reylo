select 'all' as scope, customer, count(*) as n from orders group by grouping sets ((scope, customer), ())
