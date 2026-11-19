select id, vector_extract(emb) as e from docs where emb is not null
