select count(*) as n, count(note) as noted, sum(total) as revenue, max(customer) as last_customer, avg(total) as mean from orders
