merge into users u using (select 1 as i) s on u.id = s.i when matched then update set name = 'x' returning id + 1
