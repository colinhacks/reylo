select body::integer as a, cast(body as double) as b, typed::bigint as c from docs where json_type(body) = 'NULL' and json_type(typed) = 'NULL'
