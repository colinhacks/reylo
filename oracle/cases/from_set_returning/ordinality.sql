select t.tag, t.n from people, unnest(tags) with ordinality t (tag, n)
