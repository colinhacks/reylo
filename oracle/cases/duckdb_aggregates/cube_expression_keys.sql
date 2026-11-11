select total > 15 as big, note is null as missing, count(*) as n from orders group by cube (total > 15, note is null)
