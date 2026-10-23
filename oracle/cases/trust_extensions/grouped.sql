select sum(age) as s, max(email) as m, count(*) as n from users group by email
