select jsonb_agg(distinct g) as docs, array_agg(distinct g) as shapes from shapes
