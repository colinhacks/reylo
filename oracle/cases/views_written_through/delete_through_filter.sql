delete from published where id = :id returning id, title, published_at, status
