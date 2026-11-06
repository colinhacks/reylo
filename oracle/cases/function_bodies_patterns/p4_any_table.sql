select p.id, p.title from post p where matches(p.body, :query)
