select json_build_object('k', (j::text)::json, 'n', (n::text)::jsonb) as o from doc where id = :id
