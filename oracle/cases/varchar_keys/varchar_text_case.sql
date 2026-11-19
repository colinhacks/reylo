select id from vkey where email = case when :flag then lower(:e) else upper(:f) end
