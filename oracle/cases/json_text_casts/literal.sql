select jsonb_build_object('tags', '[]'::jsonb, 'meta', '{}'::jsonb, 'none', 'null'::jsonb) as o, jsonb_build_array('[]'::jsonb, 1) as a, json_build_object('k', cast('{"a": 1}' as json)) as c
