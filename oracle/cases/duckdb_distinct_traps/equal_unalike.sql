select list(distinct s) as words, list_distinct(list(s)) as kept, list_distinct(list(i)) as spans from c
