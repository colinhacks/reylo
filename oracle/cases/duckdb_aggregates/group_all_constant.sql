select 'all' as scope, count(*) as n, sum(total) as revenue from orders group by all
