select avg(embedding) as mean, sum(embedding) as total, array_agg(embedding) as embeddings, array_agg(draft) filter (where draft is not null) as drafts from items
