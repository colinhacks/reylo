select region, count(*) as n, sum(total) as revenue, max(note) as last_note, min(customer) as first_customer, list(customer) as customers from orders group by region
