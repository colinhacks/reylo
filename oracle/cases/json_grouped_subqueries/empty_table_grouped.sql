select (select jsonb_build_object('c', count(*), 'm', max(x.v), 'a', jsonb_agg(x.v)) from nothing x group by x.id) as j
