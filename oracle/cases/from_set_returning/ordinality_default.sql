select tag, ordinality from people p, unnest(p.tags) with ordinality tag
