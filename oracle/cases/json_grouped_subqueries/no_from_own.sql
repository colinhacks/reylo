select (select jsonb_build_object('m', max(1), 'c', count(*), 'a', jsonb_agg(2))) as j from users u
