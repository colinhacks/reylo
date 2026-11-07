select p.id, p.published_at from post p where is_published(p) and p.published_at is not null
