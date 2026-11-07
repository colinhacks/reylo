select e.v from comments cross join lateral unaliased(note) e
