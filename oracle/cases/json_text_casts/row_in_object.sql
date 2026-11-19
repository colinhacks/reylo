select jsonb_build_object('doc', d) as o from doc d where d.id = :id
