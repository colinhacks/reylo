select (select x.j from lateral (select to_jsonb(c) as j from (select 1 as id) c) x) as j from json_to_recordset('[{"c": {"street": "s", "zip": 1}}, {}]') as o(c addr)
