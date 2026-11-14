update doc set label = :label where id = :id returning jsonb_build_object('was', (old.j::text)::jsonb, 'now', (new.j::text)::jsonb) as o
