select dateadd(year, n, d) as a, datediff(day, d, d) as b, datename(month, d) as c, eomonth(d) as e, datetrunc(month, d) as t from f
