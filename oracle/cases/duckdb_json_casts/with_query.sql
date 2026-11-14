with d as (select body, n from docs where json_type(body) = 'NULL') select body::integer as a, n::bigint as k from d
