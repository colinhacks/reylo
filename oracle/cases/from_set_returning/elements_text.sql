select x.value from people p, jsonb_array_elements_text(p.list) x
