select e.kind, jsonb_build_object('rows', array_agg(e)) as rows from events e group by e.kind
