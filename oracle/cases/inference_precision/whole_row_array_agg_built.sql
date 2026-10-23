select e.kind, array_agg(row(e.id, e.kind, e.payload, e.note)::events) as rows from events e group by e.kind
