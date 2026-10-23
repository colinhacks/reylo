select jsonb_build_object('name', doc -> 'name', 'owner_id', doc #> '{owner,id}', 'note', doc -> 'meta' -> 'note') as o from docs
