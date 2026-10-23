select json_build_object('c', c, 'e', e, 's', s, 'i', i, 'q', q) as j, to_jsonb(m) as row from shapes m
