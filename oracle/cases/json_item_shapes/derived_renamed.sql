select t.k from (select jsonb_build_object('id', id) as j from posts) t (k)
