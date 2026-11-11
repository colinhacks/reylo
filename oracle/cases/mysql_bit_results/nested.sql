select if(n > 0, if(n > 1, enabled, 0), 0) as a, (case when n > 1 then 1 when n > 0 then (if(n = 1, mask, 2)) else 3 end) as b from flags
