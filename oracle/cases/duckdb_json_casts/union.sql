select body::integer as a from (select body from docs where json_type(body) = 'NULL' union all select '7'::json) u
