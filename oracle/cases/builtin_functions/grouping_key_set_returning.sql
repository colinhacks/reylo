select generate_series(1, 2) as g, count(*) as n from events group by cube (g)
