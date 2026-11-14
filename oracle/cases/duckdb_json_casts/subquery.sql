select d.b::integer as a, d.k::bigint as k from (select body as b, n as k from docs where json_type(body) = 'NULL') d
