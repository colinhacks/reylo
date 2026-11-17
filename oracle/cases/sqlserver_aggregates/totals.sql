select count(*) as n, count(name) as named, sum(credit) as credit, avg(credit) as average, min(joined) as first, max(email) as last from customers
