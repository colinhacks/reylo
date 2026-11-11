select x.value, k from people p, jsonb_array_elements(p.list) x, jsonb_object_keys(p.prefs) k
