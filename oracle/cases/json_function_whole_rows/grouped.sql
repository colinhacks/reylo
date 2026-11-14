select to_jsonb(c.*) as doc, count(*) as n from bodies_for(1) c group by c.body
