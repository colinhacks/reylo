select json_build_object('arr', a.arr) as o, to_jsonb(a) as r, row_to_json(a) as t from docs_arr a
