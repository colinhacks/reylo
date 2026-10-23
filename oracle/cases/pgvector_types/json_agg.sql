select jsonb_agg(embedding) as embeddings, jsonb_agg(jsonb_build_object('half', half)) as halves from items
