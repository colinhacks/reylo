select t.id, x.o from team t left join (select id, jsonb_build_object('n', first_name) as o from member) x on x.id = t.id
