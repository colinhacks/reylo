select (select jsonb_build_object('c', count(*), 'm', max(x.v), 'a', jsonb_agg(x.v), 's', sum(x.v)) from nothing x) as j
