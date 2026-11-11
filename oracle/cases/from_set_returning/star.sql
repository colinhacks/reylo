select * from people p, jsonb_each(p.prefs) with ordinality
