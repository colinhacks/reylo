merge into one_column t using (select 1 as i) s on t.x = s.i when matched then update set x = 5 returning x
