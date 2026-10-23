select (select coalesce(array_agg(q), '{}') from posts q) as rows
