insert into items (id, embedding, draft, sparse) values (:id, :embedding, :draft, :sparse) on conflict (id) do nothing returning id, embedding, draft, sparse
