select e.key, e.value from people p, jsonb_each_text(p.prefs) e
