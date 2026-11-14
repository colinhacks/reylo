select x.j from aa o(id, c) cross join lateral (select to_jsonb(c.*) as j from (select 5 as street) c) x
