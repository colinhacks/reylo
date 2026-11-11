select kv.key, kv.value from people, jsonb_each(prefs) kv
