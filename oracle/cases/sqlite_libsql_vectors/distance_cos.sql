select id, vector_distance_cos(emb, vector(:q)) as d from docs where emb is not null order by d limit 5
