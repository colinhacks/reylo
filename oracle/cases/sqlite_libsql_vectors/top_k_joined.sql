select d.id, d.title, vector_distance_cos(d.emb, '[1,2,3]') as distance from vector_top_k('docs_emb', vector(:q), 5) as t join docs d on d.id = t.id
