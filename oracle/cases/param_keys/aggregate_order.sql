select string_agg(name, ',' order by :o::text) as names from users
