select id, vector_dims(embedding) as dims, vector_norm(embedding) as norm, embedding::real[] as parts, l2_normalize(draft) as unit from items where embedding = :q
