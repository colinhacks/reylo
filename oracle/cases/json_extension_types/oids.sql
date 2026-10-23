select json_build_object('l', l, 'o', o) as j, to_jsonb(b) as row from blobs b
