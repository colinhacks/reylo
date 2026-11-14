select if(n > 0, enabled, 0) as a, case when n > 0 then enabled else n end as b, if(n > 0, enabled, 1.5) as d, if(n > 0, mask, n) as l from flags
