select list(distinct name) as names from tags union all select list(distinct name) from tags where id > 2
