update published set published_at = null where id = :id returning id, published_at
