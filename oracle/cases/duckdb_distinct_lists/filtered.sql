select list(distinct name) filter (where id > 1) as later, list_distinct(list(name)) as kept from tags
