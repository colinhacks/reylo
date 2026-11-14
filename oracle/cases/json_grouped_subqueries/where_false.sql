select (select jsonb_build_object('c', count(*), 'm', max(p.n), 'a', jsonb_agg(p.id)) from posts p where false) as j
