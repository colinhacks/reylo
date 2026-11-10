select n, min(body)::integer as low, min(n)::bigint as least, count(*)::integer as total from docs where json_type(body) = 'NULL' group by n
