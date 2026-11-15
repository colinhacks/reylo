select l from (select list(distinct x) as l from c union all select list(distinct y) from c)
