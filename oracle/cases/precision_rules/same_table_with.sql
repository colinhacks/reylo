with c as (select id, 'v' as v from member) select m.last_name, c.v from member m left join c using (id)
