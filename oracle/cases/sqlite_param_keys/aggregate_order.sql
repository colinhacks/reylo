select group_concat(name, ',' order by :o) as names from users
