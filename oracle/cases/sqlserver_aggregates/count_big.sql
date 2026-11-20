select count_big(*) as n, sum(cast(qty as bigint)) as units from lines
