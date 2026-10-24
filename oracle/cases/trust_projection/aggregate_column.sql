select sum(n) as total, count(*) as c, coalesce(sum(n), 0) as z, (select coalesce(avg(n), 0) from s) as a from s
