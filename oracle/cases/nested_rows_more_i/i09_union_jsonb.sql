select x.j from cc, lateral (select to_jsonb(c) as j from replies c union all select '{}'::jsonb) x
