select greatest(id, null) as a, least(first_name, null, 'x') as b, greatest(manager_id, null) as c, least(manager_id, id) as d, (select greatest(count(*), 1) from member) as e from member
