select status, count(*) as n from orders group by status having count(*) > 1
