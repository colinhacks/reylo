merge trig as t using (select :id as id) as s on t.id = s.id when matched then update set v = 1;
