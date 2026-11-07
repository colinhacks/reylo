select p.id, p.title, p.status from post p where visible_to(p, :viewer::int8)
