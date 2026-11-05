with recursive c(i) as (select i from s union all select i from c where false) select i from c
