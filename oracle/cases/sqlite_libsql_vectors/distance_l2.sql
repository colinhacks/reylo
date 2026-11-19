select id, vector_distance_l2(emb, '[1,2,3]') as d from docs where emb is not null
