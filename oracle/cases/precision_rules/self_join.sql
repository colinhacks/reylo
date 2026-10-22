select m.id, m.first_name, b.first_name as boss, b.id as boss_id from member m left join member b on b.id = m.manager_id
